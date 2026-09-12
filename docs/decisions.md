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

**D11 — A saved game is `l2_kingdom::save` plus ten fields; it lives under the user's
profile; and its extension is `.l2sav`.**

D10 settled the *world's* encoding. A saved **game** is that file with a short prefix —
the ten plain fields `l2_game::Game` adds on top of `Kingdom`: the local player, the map
slot, the realm colours, the selected county, the county anchors, last turn's treasuries,
the last season's report and the turns played. `l2_game::save` writes that prefix through
the same `Canonical` and then calls `l2_kingdom::save::encode` for the rest, unchanged, so
there is exactly one kingdom encoder in the workspace and no way for a save and a lockstep
checksum to disagree about how an integer is spelled.

**Two version numbers, and both refuse rather than guess.** The prefix has its own, and the
world keeps `l2_kingdom::save::VERSION`. Either being unfamiliar names itself and stops;
neither is ever read on the assumption that the fields happen to line up.

**Saves live in `%APPDATA%\open-lords2\saves`** (`$XDG_DATA_HOME/open-lords2/saves`
elsewhere), overridable with `LORDS2_SAVES`, and the directory is created on the first
write. Three places were considered and rejected: *inside the game install*, which rule 2
forbids and which is where the original's volatile `lastturn.sav` already lives; *inside
the repository*, where `.gitignore` and the census both refuse it; and *beside the
executable*, which works for a portable build and fails for an installed one because
`%PROGRAMFILES%` is not writable by the player.

**The extension is deliberately not `.sav`.** Theirs is an unversioned memory dump whose
schema comes out of `Lords2.exe` and which we read as an oracle; ours is a versioned format
we write. Writing *their* format is a separate job that nothing needs yet. `.l2sav` also
keeps `*.sav` in `.gitignore` meaning exactly one thing.

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

**C30 — Four county fields were in no save and in no checksum, and the test written to
catch exactly that did not, because the catching was a hand-written list.**

`l2_kingdom::save` did not encode `County::labour_wanted`, `labour_useful`, `labour_share`
or `industry_share`. A kingdom decoded from a save came back with `County::new`'s defaults
in all four — `labour_share` at 40/15/15/15/15, `industry_share` at 25 — whatever the
county actually held.

**Two of them are simulation state, not display.** `labour_useful` is the ceiling the
labour allocator (`FUN_0044F6E7`) fills each job up to before dropping the remainder into
*Idle townsfolk*, and `labour_share` is its only instruction about where people should go.
So this was a hole in the **lockstep checksum** (`docs/netcode.md` §5) and not only in the
save: two peers could disagree about all four and the per-tick digest would agree, because
the digest is `Canonical::hash_of(kingdom)` and runs through the same `Encode` impl the
save does. One encoder is D10's whole point, and it cuts both ways — a field missing from
it is missing from everything at once.

**How it was found, and why it took a second format to find it.** The game save's round
trip over the England turn-one position produced a kingdom whose checksum was *equal* and
whose `PartialEq` was *not*. That combination can only mean a field outside the encoding,
and it is not reachable from inside `l2-kingdom`'s own suite: `tests/save.rs` compares
decoded against original with `assert_eq!` too, but over `furnished()`, a kingdom this
file builds by hand — and `furnished()` never sets any of the four, so both sides held the
default and matched.

**The guard that should have caught it was a list.** `every_part_of_the_state_reaches_the_bytes`
exists precisely for fields that round-trip perfectly because neither side writes them; it
mutates a field and requires the bytes to move. It has forty-odd entries, hand-written, and
it simply had no line for these four. **A completeness check that enumerates what to check
is only as complete as the enumeration** — the same shape as C25 and C29, where the evidence
was already in the tree and the label on top of it was not re-read. The four lines are added
and `l2_kingdom::save::VERSION` is 5; a version 4 save is refused rather than loaded with the
defaults, because there is no way to recover what the fields held.

**What would actually close it** is deriving the field list from the struct rather than
retyping it — a `Kingdom` walked by reflection, or an encoding generated from the
definition. That is not written, and until it is, this correction is the reason to add a
line to that list every time a field is added to `County`.

**C31 — A battle had no end. The function that ends one was in no document, and the two
rules we thought we had about the end of a battle were both somebody else's.**

Found while wiring the campaign–battle seam. `crates/l2-sim` was a frame loop with no
victory test, no outcome and no return: you could get *into* a battle and there was nothing
to come back from. The original's answer is **`Battle_CheckOutcome` (`0x00477DFC`)**, 1,225
bytes, and it was in neither `symbols.json` nor `hypotheses.json` — so nothing in this tree
pointed at the one function that decides whether a battle is over.

**What it says, and it is shorter than anyone expected.** A field battle ends exactly two
ways: one side's living-men counter reaching zero, or a withdrawal flag, which is tested
first and outranks annihilation. **No morale break, no rout threshold, no clock.** Three
further arms are sieges, including an *assault repulsed, repeat* that resets the breach and
approach scores and carries on. The outcome then picks one of seven `L2.eng` group 82
heading/body pairs, and `Battle_WriteBackCasualties` (`0x0047F474`) — the *"path not traced
here"* `armies.md` §7 left open — turns the surviving figures back into troop counts.

**Two corrections ride along, and both are the same mistake in different places.**

1. **`docs/armies.md` §7 and `symbols.md` both said the ≥ 50-men survival rule was an
   *autocalc* rule.** It is not. Its gate is `g_battleWithdrawal` (`0x0056D5C8`), and
   `Battle_AutoResolve`'s **first statement clears it**. The flag is raised in exactly one
   place, `UnitOrder_SiegeAttKnight`, when an all-knight AI besieger faces an unbreached
   wall. So it is a *siege-withdrawal* rule, and under autocalc the loser is always
   destroyed. Four sites in the whole binary touch that flag; reading all four is what
   settles it, and reading one is what produced the wrong sentence.
2. **`armies.md` §8.1 still said a county is attacked "by stepping onto its castle tile".**
   C25 corrected `0x40` from castle to county town two entries ago; the sentence was written
   before that and kept the old label. `L2.eng` group 30 says it outright — *"Your troops
   may capture a castleless county by attacking its county town"* — and `battle-before.sav`
   has the player's army sitting **on** county 3's castle tile with no battle at all.

**What paid for this.** The battle fixture triple, which `armies.md` had declared could not
exist. `Battle_AutoResolve`'s ladder, read out of `Lords2.exe` at `0x004DE710`, reproduces
`battle-after.sav` to the man: strengths 926 against 1044, ratio 112, the ladder's second
rung 20 %, and 20 % of 122 peasants and 60 archers is the 36 people county 3 gets back.
Four independent numbers had to be right at once.

**The lesson is not C3's and not C25's.** Nothing here was a plausible story assembled from
decompiler output; the readings were *correct about the code they had read*. What was wrong
was the **scope** of that reading — one branch stood in for a function, one function stood
in for a subsystem, and a flag named in one place was assumed to mean the same thing in
another. **A `[V]` on a branch is not a `[V]` on the rule the branch belongs to**, and the
cheapest defence is the one this correction used: grep every site that touches the flag, and
count them.
**C32 — Nothing detected a winner, and the victory condition everybody had written down
was a paraphrase that changed what it says.**

Found while wiring the ending. `Realm::is_eliminated` and `Realm::in_play` were implemented
and tested; nothing read either as an ending, so a game could not be won, lost, or finished.
The chain the plan named — `Realm_RecountStrength` → `Score_RankRealms` → the outcome byte →
screen `0x1C` — is real and now implemented. **Four things it does are not what the plan said,
and all four were checked against the binary rather than argued about.**

1. **"`Score_RankRealms` fires group 225 when the trailing realm equals the leader" is a
   sentence that reads as a scores comparison and is not one.** `g_rankLeader` and
   `g_rankTrailer` are the first and last *realm indices* left in the sorted ranking table
   after every eliminated realm has been struck out of it. Two indices are equal exactly when
   one realm is left. The condition is the ordinary one, written as a table scan; the plan
   warned it "reads strange", and the strangeness is entirely in the paraphrase. It also
   passes with **nobody** in play — both are then 0 — and the original goes on to crown
   `g_realms[0]`, the slot that is never a realm.
2. **That is not how a person wins, though.** The ordinary human victory is in
   `Msg_DrawWindow`: any ending message displayed while `g_opponentsRemaining` is zero
   *enqueues* group 225 at the local player, and the win lands when that message is shown.
   Killing the last AI raises group 194 about **that AI**, and displaying 194 with nobody
   left is what wins the game. A reading that stopped at `Score_RankRealms` would have
   produced a game that could only be won by the second, redundant path.
3. **`AI_RunTurnStep` is not skipped for a human, and `symbols.json` said it was.** The
   `isHuman` test guards the fourteen handlers and the counter's increment, not the step-0
   initialisation above them. Step 0 — the strength recount, and therefore *the detection of
   the human's own defeat* — runs for every realm. Skipping humans there is the one way to
   build a game that cannot be lost, and our `ai::run_step` skipped them. Entry corrected.
4. **The asymmetry is on "is this the local player", not on "is this a human".** The local
   player gets group 224, an AI gets 194, and a **second human in a network game is
   eliminated in silence**. Three arms, and only two of them are about humanity.

**Two bugs of ours that only the ending could expose.** `ai::all_realms_done` asked every
realm for its sentinel rather than every *in-play* realm, which is fine while `begin_turn` is
the only writer of `in_play` and hangs phase 4 the moment a realm is eliminated during it —
that is, on the turn the game ends. And `County_ChangeOwner`'s recount of the outgoing owner
happens one statement *before* the owner is written, so it can never eliminate anybody; a
realm losing its last county survives until its own step 0. Both are now reproduced and named.

**C33 — The score's gold bracket is dead code from the second rung up, and we had
implemented the table's intent instead of the executable's behaviour.**

`docs/rules.md`, `docs/kingdom.md` §8.3 and `l2_kingdom::tables::score_gold_bracket` all gave
the treasury bonus as +50 over 2,000, +100 over 5,000, +200 over 10,000. The shipped
`Score_RankRealms` tests the **smallest threshold first**:

```text
0049aed1  cmp  [eax+57c018], 2000
0049aedb  jle  0049aefb
0049aee1  add  [eax+57bf50], 50        ; and then jmp straight to the next realm
```

so the 5,000 and 10,000 arms are reached only by a treasury that has already failed
`> 2000`. **The bonus is 0 below 2,001 and 50 above it**, and a full treasury is worth one
castle rather than four. Read out of the instruction bytes, because the decompiler's nesting
is exactly the kind of evidence C3 warns about and this claim deserved better than one
rendering of it. The stock ruleset now pays 50 on all three brackets and keeps the
thresholds, so a ruleset that wants the ladder the table was designed for changes three
numbers.

**The lesson is C13's, sharpened.** Every one of these six corrections came from a *summary*
of a function — the plan's, `symbols.json`'s, or a doc comment's — that was faithful to the
shape of the code and wrong about what it does. A paraphrase is a lead. The bytes are the
finding.

**C34 — A tie in the secession pass was said to go to the lowest block index. It goes to
the highest. The operator was read correctly and the conclusion drawn backwards.**

`Territory_SecedeMinorBlocks` (`0x0044AE51`) picks the block a realm keeps with

```c
for (i = 0; i < 17; i++)
    if (blocks[i].owner == realm && best <= blocks[i].population) { kept = i; best = ...; }
```

and `symbols.json` and `docs/kingdom.md` §6.1 both said *"ties go to the lowest block index,
**because** the comparison is `<=`"*. The `because` is where it went wrong: `<=` is exactly
what makes a later equal block **overwrite** the incumbent. `<` would have given the lowest.

It is a one-word error in a sentence whose reasoning is visible, which is what makes it worth
a number: **the citation was carried forward twice without anyone re-deriving it**, once into
`symbols.md` by the generator and once into the prose. The check that catches this class is
cheap and was not run — write the tie-break down as a test with two equal blocks and read
which one survives (`territory.rs`,
`an_equal_population_hands_the_realm_the_higher_numbered_block`).

Three smaller readings in the same pass are corrected with it, all in the same direction —
the code says more than the summary did. The realm loop is **1 … 5 guarded on
`strength != 0`**, so an eliminated realm is skipped and realm 0 is never considered. The
key is the **sum of the block's members' populations**, not its county count. And the
message is suppressed unless the realm held more than one block, which is a second guard on
top of "something actually seceded".

**And the honest note about the anchor, which is not a correction but belongs beside one.**
This mechanic still has no data-side oracle: every realm in all six fixtures owns exactly one
county. A player has now confirmed from memory that cut-off counties secede in play, and that
is what moved it from unanchored to corroborated and got it implemented. Recorded in
`docs/kingdom.md` §6.1 and `docs/rules.md` §5a **as a recollection plus two `L2.eng`
strings**, because writing it down as anything stronger is how C5 and C8 happened.

**C35 — Phase 2 is not army movement, and nothing on the campaign map moves inside a phase
at all.**

`kingdom.md` §3.1's table has read *"phase 2 — army movement, including battle resolution —
waits for armies (unit type 1)"* since the phase machine was first traced, and
`crates/l2-kingdom`'s `Phase::ArmyMovement.wait()` was written from it as
`PhaseWait::Units(UnitKind::Army)`. Both are wrong, and the second was wrong in a way that
would have been very hard to see: with no units in the array the predicate is vacuously
true, so the phase settled correctly for the wrong reason for as long as there was nothing
to move.

`Turn_Tick`'s phase-2 arm, in full:

```c
if (g_turnPhaseStep % 100 == 2) {
    if (Siege_TickPhase() == 0) Turn_AdvancePhase();
    else { DAT_0055403C = 0; Siege_LaunchAssault(g_siegeCursor); }
}
```

No unit sweep, no type-1 predicate, nothing that reads `moving`. The three phases that
*do* wait on units call `FUN_004A4F5B(type)` (3 and 6) or `FUN_004A4E3D(2, 6)` (5), and
phase 2 calls neither; its first step, `Siege_StartPhase` (`0x004A82B9`), touches only
`besiegedBy`, `besiegingCounty` and `garrisonUnit`. Phase 2 is the three-function siege
machine `armies.md` §2.1 already described correctly — validate, build, assault — and §3.1's
row was never reconciled with it. Two documents in this tree disagreed about the same phase
and the newer, more specific one was right.

**The larger half.** Asking where army movement *is*, if it is not phase 2, answers a
question nobody had asked: `Units_Tick` (`0x004650B0`) has exactly one call site, and it is
the frame loop, immediately after `Turn_Tick` —

```c
if ((g_battlePhase == 0) && (ticksDue != 0)) { FUN_0040490D(); Turn_Tick(); Units_Tick(); }
```

— and it never reads `g_turnPhase`. **Every unit that is walking takes a step on every tick
of the whole turn.** The phases only *originate* the game's own move orders — transports in
3, mobs in 5, merchants in 6 — and then wait for them to stop; a player's order does not
involve a phase at all, because `Unit_OrderMove` sets the unit walking directly.

This is a structural fact rather than a detail, and building the mover as a phase handler —
which is what §3.1 invited, and what this work started out doing — would have produced a
game where an army ordered during the player's turn stood still until the phase came round
again. `crates/l2-kingdom/src/units_tick.rs` is the reading, `kingdom.md` §3.1 is corrected,
and `PhaseWait::Sieges` replaces the unit wait on phase 2.

**What made it findable.** The phase-2 row carried **[D]**, derived from "the unit type each
phase waits on", and §3.1 says so in as many words. The derivation is right for 3, 5 and 6
and the marking was honest; what was missing was that a **[D]** row disagreeing with a
**[V]** section elsewhere in the same repository is a contradiction somebody has to spend,
not a difference of emphasis. C13's shape once more: the summary and the branch disagreed
and the summary was load-bearing.

**C36 — `AI_ManageFields` and `Ai_ManageCountyFarms` are two functions, and treating them as
one hid two of the five farming styles and left the AI's field expansion doing nothing at
all.**

C28's shape again, and this time the collapsed name reached the code rather than a document.
`crates/l2-kingdom/src/ai.rs` gave AI turn step 5 as *"`AI_ManageFields` (`0x0049DD01`)"*,
dispatching into *"one of three labour allocators"* that *"were not traced"*. Every clause is
wrong in a different way:

| the claim | what the corpus says |
|---|---|
| step 5 is `AI_ManageFields` | step 5 is **`Ai_ManageCountyFarms`** (`0x0049DD01`). `AI_ManageFields` is `0x0049DFC6` |
| `AI_ManageFields` is the AI's pass | its **only** caller is `AI_ManageFields(0)` in turn phase 1 — the **unowned** counties |
| three allocators | **five**: `0x004A4052` / `0x004A42E3` / `0x004A440F` for an AI realm, `0x004A3C67` / `0x004A3ED3` for the neutral counties |
| they were not traced | all five decompile cleanly and are 300–660 bytes each |

The cost was not only the missing behaviour. A briefing built on the collapsed reading told a
previous agent that **`FUN_004A3C67` is "the AI's farming style"**; it is the one allocator
**no AI realm can reach**, and the wrong version had already been propagated once before it
was checked. The check that settles it is two lines of `grep`: `AI_ManageFields` has exactly
one call site and its argument is the literal `0`.

**And the field ladder had never added a field.** `AI_FIELD_LADDER` reads as *"give the
county another field"* and this crate implemented it by adding one to `County::fields_fallow`.
Since the field counts became **tile-derived** (`crate::field`, save version 4) that counter
is recomputed from the map on the next pass, so **every field the AI ever ordered evaporated
before it could be farmed**. The original paints terrain `0x19` onto a *wasteland tile*
(`Field_OrderReclamation`, `0x0044C6C4`) — and a tile already reclaiming spends a place in the
quota without anything happening, so a county told to add one while one is under way adds
nothing at all. Two rules, both wrong, both invisible because nothing downstream read the
result: `docs/audit.md`'s C26 exactly.

Three smaller findings came out of the same read and are worth having on the record because
each of them is a rung of a ladder that can never fire:

* the Winter grain quota's `fertility < -50` branch sits **after** `fertility < -20`, so a
  ruined county gets the same one-field discount as a tired one;
* the AI's ration ladder tests dairy above store, and because its store term counts the herd
  **twice**, its Triple dairy rung can never change an answer and its Double rung can only
  ever *lower* the level — a cattle county is fed Double where a grain county with the same
  food is fed Triple;
* turning *Advanced Farming* **off** makes the AI plant far **more** grain, because the
  option's `else` limb replaces the whole fertility ladder with `total - 1` / `total - 3` /
  `total / 2`.

`0x004A13A6` was corrected in the same pass and is a straight mis-naming rather than a
collapse: `docs/kingdom.md` §3.2 had step 13 as *"offer or break an alliance"*. It is
**`AI_Taunt`** — a two-stage gloat at the human, on a timer, with no effect on any alliance.
Alliances are step 2.

**C37 — `anchor.js` was reading two of the five `L2.eng` primitives as zero call sites,
because a rename in `symbols.json` silently unhooked its table. A key that matches nothing
looks exactly like a primitive nobody calls.**

`anchor.js`'s `ENG_CALLS` table keys on the *decompiled* function name, and `symbols.json`
named `0x004017BF` `Eng_CopyString`. `decompile-all.ps1` duly emits that name, and the
table's `FUN_004017bf` key stopped matching anything at all. Nothing failed. The scanner
just returned a number two groups smaller than the truth — 65 instead of 67 — and the two
it lost were group 7, the four lord titles, which is how new-game setup names the AI, and
group 89.

Two lessons, and only the second is new:

1. This is `docs/method.md` §4's *"a tool that is wrong is worse than an analysis that is
   wrong"* again, and the same fix applies: **the count was re-derived by a scanner
   written from the corpus rather than from `anchor.js`, and the two now agree
   set-for-set, all 67 groups.** That agreement is the check. Renaming a symbol is
   therefore not a `symbols.json` edit; it is a `symbols.json` edit plus a sweep of every
   tool that keys on a name.
2. **A number quoted off a tool's printed output is not the tool's answer.**
   `anchor.js strings` shows the top 40 functions by size and stops, so counting distinct
   groups off its output gives 40 — a third wrong number, from a correct tool, with no
   bug involved. The `--limit` was in the usage text the whole time. When a census is the
   point, check whether what you are counting is the result or the *display* of the
   result.

Recorded because three different "how many groups are reached" figures were in
circulation at once — 40, 52 and 65 — and only one of them came from anything wrong. The
52 could not be reproduced from any query and is written down as unexplained in
`docs/formats/eng.md` §5.1 rather than quietly dropped.

**C38 — C31 stopped one branch short. A besieger that loses a fight but still has men is
not destroyed, and that rule has no flag on it.**

C31 read `Battle_ReturnToCampaign`'s loser branch, found that the ≥ 50-men survival rule is
gated on `g_battleWithdrawal` rather than on the autocalc, and concluded — correctly — that
*"under autocalc the loser is always destroyed"*. It read one `if` and stopped at it. The
branch is two nested tests:

```c
if (loser.besiegingCounty == 0 || loser.menTotal == 0) {
    if (withdrawal) {
        if (loser.menTotal < 50) { message 0x120; Army_Destroy(loser); }
        else                       loser.besiegingCounty = 0;
    } else Army_Destroy(loser);
} else loser.besiegingCounty = 0;          /* <- the outer else, which C31 never reached */
```

**The outer `else` is the rule a siege needs and neither `armies.md` §7 nor C31 has it**: a
loser that is still linked as a besieger and still has living men keeps them, and all that
happens is that its siege is lifted. C31's conclusion about the autocalc is *true* and its
reason was wrong — under autocalc the loser's men are set to zero, so the outer test passes
and the inner one runs. The rule is reachable only from a **fought** siege, where
`Battle_CheckOutcome`'s *assault repulsed* and *siege lost* arms can end a battle with the
besieger still standing. So a repulsed assault costs an army its siege and not its life,
which is why sieges are a war of attrition rather than a coin toss.

**And the shipped `Readme.txt` describes the inner rule as something the code does not.**
*Retreats (pg82)*: *"Armies that retreat will suffer some casualties. Any army that would
have less than 50 men after retreating is eliminated instead."* That is exactly the inner
branch, stated as a general rule about retreating. In the shipped binary
`g_battleWithdrawal` is written in **one** place — `UnitOrder_SiegeAttKnight`, an all-knight
AI besieger giving up on an unbreached wall — so the rule the errata generalise is one the
player can never trigger. The errata are authoritative about *intent*; the code is what
shipped. Both are recorded, and `crates/l2-kingdom`'s `return_to_campaign` takes
`withdrawal` as a parameter so the branch is reachable honestly rather than by inventing a
retreat.

**The lesson is the same shape as C31's own and it is worth stating twice, because C31
stated it and then fell for it.** *"A `[V]` on a branch is not a `[V]` on the rule the
branch belongs to."* C31 wrote that sentence about a **flag** — grep every site — and did
not apply it to the **control flow** around the flag it had just read. Counting the sites
that touch a global is not the same as reading the function to its closing brace.

**C39 — The completeness check for the lockstep checksum was a list, so it kept finding
nothing. Deriving it from the struct definitions found twelve more fields in neither the
save nor the digest, one of which is the entire diplomatic matrix.**

C30 recorded four `County` fields missing from `l2_kingdom::save`'s `Encode` impl — and
therefore, since `docs/netcode.md` §6's per-tick digest is `Canonical::hash_of(kingdom)` over
that same impl, missing from the lockstep checksum. It restored the four, and it said in as
many words that **the mechanism was not fixed**: `every_part_of_the_state_reaches_the_bytes`
was a hand-written enumeration of ~130 mutations and a hand-written enumeration cannot fail
for a field nobody listed.

**It then failed three more times, each in a way C30 predicted.** Three commits after C30 the
four lines had to be added to the enumeration by hand with nothing forcing it. The next
branch added two saved fields and did not add them either. Matching the list against `County`
by hand turned up **six** fields outside it — all six encoded correctly, which is luck rather
than the test working. And matching it against `Realm` turned up **eleven that were in neither
`Encode` nor `Decode`**, including `pairs`: standing, alliance, grudge, at-war and the gift
history between every pair of realms. Two peers could have diverged on the whole diplomatic
state of a game and every checksum they exchanged would have reported agreement. `Unit`'s
`defence_mark` — which decides whether winning a battle also wins the county — made twelve.

**What replaces it is two halves, and neither is a list.**

1. **A fixture with every field holding something other than its default**, round-tripped and
   compared with `#[derive(PartialEq)]` — the only exhaustive reader of a struct this project
   has. Saturation is the whole point: a field that *both* sides ignore round-trips perfectly
   when the source also holds the default, which is exactly why all four C30 fields survived
   the old round-trip tests. Over a saturated fixture that single `assert_eq!` *is* the
   completeness check.
2. **A census that derives the field list from the source.**
   `every_field_of_the_state_is_furnished` parses every `struct` in `crates/l2-kingdom/src`,
   walks the types from `Kingdom`, and requires each field it finds to be furnished. A field
   added tomorrow fails by name. Its two exemption tables have the right polarity — inclusion
   is the default, a line is a claim with a reason attached, and a line naming a field that no
   longer exists fails too, because a stale exemption looks like a decision and covers
   nothing.

Twenty-five ablations were run to check that this is not theatre: each of the four C30 fields
alone and together, each of the six later `County` fields, each of the eleven `Realm` fields,
`defence_mark`, and two fields *inside* `Pair` — deleted from the encoding one at a time.
**All twenty-five fail the round trip.** The hand-written enumeration is deleted, along with
`tests/save_gap.rs`, which pinned eleven of them; leaving either beside a derived check is how
the derived one rots.

**A second property, easy to miss, and the original has the bug.** A *field* can go missing
from a record; a whole *record* can go missing from the walk over an array, and no amount of
field checking sees that. `no_record_slot_is_silenced` loops over the array lengths instead.
The original game has exactly this defect in its own multiplayer sync: `Sync_BuildDigest`
(`0x00440231`) fills eight per-block digest bytes and, as its last statement before summing
them, overwrites byte 7 — the battle-unit block — with the constant 1, so that block's
divergences are silenced in the shipped build. **This class of bug is not a mark of our
carelessness — it is what happens whenever a completeness check is written by hand, in 1996
or now.**

**The generalisable rule, and it is not only about saves: completeness must be derived, not
remembered.** Whenever a test enumerates what to check, the enumeration is the thing that
will be wrong, and it will be wrong silently and in the safe-looking direction. C25, C29 and
C30 are the same shape. Where a derive macro is unavailable — `l2-kingdom` is
dependency-free on purpose — reading the source in a test is the available substitute, and
`crates/l2-testkit/tests/census.rs` had already established it as the house style.

**The same disease one file over, and two branches independently caught it.**
`l2_kingdom::save::VERSION` collided in four consecutive merges: two branches bumped 5 → 6
with different field sets (so the merged encoding had to become 7), then two collided at 7,
then two at 8, then this branch's entry had to move 8 → 9 → 10. Nothing checked it; the
changelog above the constant was the only guard, and only if somebody read it. This work and
C36's were both written to fix that, arrived at the same rule and even the same test name —
`the_version_is_ahead_of_its_own_changelog`, requiring the `* N —` entries to be `1..=VERSION`
with no gap and no repeat, exactly as `tools/decisions/corrections.js` does for these
correction numbers.

**Only one survived, deliberately.** C36's landed first and reads the changelog through
`include_str!` rather than a path, and additionally requires every entry to carry a
description; this branch's duplicate was deleted rather than kept alongside it. Two tests
asserting the same property is how one of them rots, which is the whole subject of this
correction. The fourth collision, this one, is the first that a machine caught rather than an
integrator reading a doc comment — the check working on the day it was written, on the branch
that wrote it.

**C40 — A unit standing in the way was treated as an obstacle for everybody. Merchants and
transports walk straight through, and so does anything walking through a merchant.
`Unit_EnterOccupiedTile`'s three opening guards were paraphrased in two places and
implemented in neither.**

`docs/armies.md` §2.7 read *"types 3 and 4 return immediately — merchants and transports are
non-combatants"*, and `units_tick::classify_occupied` opens its ladder with *"the mover is a
merchant or a transport — **walk through**"*. Both describe the guards. Neither *does* them:
`movement::try_enter` returned `Entry::Occupied` for every occupied tile, `Entry::ends_the_move`
makes that the end of the move, and `classify_occupied` then reported `Contact::Blocked` for
the two cases its own comment says walk through. The function opens:

```c
local_8 = (flags & 1) ? 3 : 1;               /* an ordinary road or open step */
if (mover.kind == 3) return local_8;
if (mover.kind == 4) return local_8;
if (occupant.kind == 3) return local_8;
...the ladder...
```

**3 and 1 are not stop codes.** `Unit_StepOnce` returns early only above 4, so those three
returns are the tile classified as ordinary ground — the mover steps onto it and keeps going,
at the tile's ordinary cost and *without* the field surcharge, the trample or the castle
capture the tile's other bits would have earned, because the occupancy test happens first in
`Unit_TryEnterTile`.

It stood because **nothing in the workspace had ever put two units on the map at once**: the
seam that loads a save dropped `g_units` on the floor, so every unit in existence was one a
test had built by hand for a scenario about one unit. Importing the block found it in two
seasons — the England position's six merchants converge on the road junction between counties
11 and 12, each stops in front of the next, and none of them reaches a county again. In the
original they pass through each other.

The class is a new one and worth naming: **a guard that everybody described and nobody
executed.** Two documents and a doc-comment all carried the words "return immediately" and
"walk through", which read as *this is handled*; the code path they described did not exist,
and the prose was accurate enough that re-reading it never raised the question. C13 is the
summary disagreeing with the branch; this is the summary agreeing with the branch and the
*code* agreeing with neither.

The fix is one function, `movement::pass_through`, applied where the tile is classified, so
rungs 1 and 2 of `classify_occupied`'s ladder are now unreachable by construction rather than
by comment. The check is cheap and is now written down —
`a_merchant_walks_through_whatever_is_standing_in_its_way` walks six mover/blocker pairs and
asserts which four pass.
**C41 — `L2_maps.dat` is not what the game draws. Every county town on the map was rendered
as four stone quarries, because that is literally what the file says and the original
overwrites it before the first frame.**

A player looked at his county and said *"there is no town square, just 4 quarries where it
should be"*. He was describing the pixels exactly.

The town's 2 × 2 block is stored as bank `0x0c` (`Town1a.pl8`), frames 0 … 3
(`maps-layers.md` §1.2's own worked example). We rendered the file. **`Town1a.pl8` frames
0 … 3 are the quarry artwork** — four dark excavated pits — and that is not a coincidence
of appearance, it is how the game itself identifies a quarry:
`County_PlaceResourceSites` (`0x00468E61`) scans for `bank & 0x1c == 0x0c` and reads the
frame, `frame == 0` giving stone, `20` wood and `30` iron.

The stored frames are a placeholder. `Counties_PlaceSites` (`0x00468D4F`) runs
`County_FindTownTile` first, which calls `FUN_0046AC22('/', 2, county, 0x0c, 0)` and stamps
`bank &= 0xE3; bank |= 0x0c; frame = quadTable[part] + base`, and the population pass
re-stamps it **every season**:

| county population | town frames |
|---|---|
| `< 0x321` (801) | 47 … 50 (`'/'`) |
| `< 0x4B1` (1201) | 51 … 54 (`'3'`) |
| otherwise | 55 … 58 (`'7'`) |

So the town is a **village whose size is its population**, redrawn as the county grows, and
the frames the file holds are never on screen in the original at all.

**What we do now, and what it costs.** `l2-view` gains a sparse
[`campaign::Overrides`](../crates/l2-view/src/campaign.rs) plane — `None` everywhere means
"draw the file" — and the map screen fills in the towns from the counties' populations.
That is one of the rewrites `Counties_PlaceSites` performs; the others (the castle into
bank `0x10`, the four resource sites) are not reproduced, and any of them may turn out to be
drawing a placeholder too. The plane exists so that finding the next one is a fill rather
than a redesign.

Two documentation errors fall out of it, both in `docs/formats/maps-layers.md`:

- §2's table labels bit `0x40` *"castle site"* and `0x80` *"settlement"*, which is C25's
  swap surviving in the one place that mattered most, because that table is the reader's
  map of the file. `County_FindTownTile` (`0x00467FD1`) scans `0x40`; `County_FindCastleTile`
  (`0x00468121`) scans `0x80` and stamps terrain `0x14`.
- §5.4 explains the 47 … 50 rewrite as *"the game was started with Starting Castle: keep"*.
  It is not a castle option; 47 / 51 / 55 are the three **village sizes** and the selector is
  the county's population.

**What made it findable, and what did not.** Nothing in the workspace could have caught
this: two of our own implementations agreeing proves nothing, and here we had only one, and
it agreed with the file. It took somebody who has played the game looking at a screenshot.
That is the third time — C21 and C22 were the others — and the pattern is now explicit
enough to state: **a renderer verified against its input file is verified against the wrong
thing.** The oracle is the running game's framebuffer, and until we diff against it, a
player's eye is the only instrument we have.

**C42 — The county strip's happiness figure was right-anchored on a coordinate that is a
left origin, and its two captions were dimmed where the original draws them in the same
colour as everything else.**

`CountyStrip_Draw` (`0x0040F7D3`) draws the two numbers with the same function and the same
argument shape:

```c
Ui_DrawNumber(pop,       ' ', " ", 0x1fc, 0xbd, &g_fontSmall, 0x3f);
Ui_DrawNumber(happiness, ' ', " ", 0x25a, 0xbd, &g_fontSmall, 0x3f);
```

**`Ui_DrawNumber` has no anchoring argument.** The calls differ in their value and their x
and in nothing else, so if `0x1FC` is where the population starts — and it is, immediately
right of the plate's peasant icon — then `0x25A` is where the happiness starts. We drew it
so that it *ended* at 602, which put a two-digit number on top of the plate's heart and
would have walked a three-digit one further left still. `docs/screens-county.md` §2.1's
table gives both as bare coordinates, and the ambiguity was resolved by guessing.

Two more from the same function, both in §2.1:

- **The county name is `g_fontBody` (`Fntl2_14.pl8`)**, not the strip's 9-pixel font. §2.1
  ends *"all of it in the 9-pixel font, which is the only place that font is used"* — the
  second half is right and the first is not. `DAT_005AEA40` is set to 1 *after* the name and
  cleared after the numbers, so the name is embossed and the numbers are flat.
- **The *Tax* and *Ration* captions are colour `0x3F`**, the same as every number beside
  them. Ours were drawn in the dim ink, which against the plate's own texture is very nearly
  invisible — a legibility bug produced entirely by inventing an emphasis the original does
  not have.

**What made it findable.** A screenshot of the running game, cropped and enlarged four
times. The layout was already asserted to the pixel by
`the_county_strip_shows_the_saves_numbers_where_the_original_puts_them`, and that test
passed throughout, because it asserted the coordinate we had chosen rather than the one the
call gives. C28's lesson at a smaller scale: a test written from the same reading as the
code confirms the reading, not the behaviour.

**C43 — The campaign sidebar's five buttons were one button of ours, drawn over.**

`crates/l2-game/src/screens/map.rs` said, of the 162 × 30 strip at (478, 430): *"the
original puts a status line here, not a button; we use it to open the county panel"*. The
original puts **five buttons** there — `g_sidebarButtons` (`0x004DC680`), dispatched by
`Sidebar_Button` (`0x0043AE30`) — and the artwork for all five is painted into `Misc_cty`
frame `0x39`, which we were already drawing and then writing *COUNTY PANEL* and a status
line across.

| id | rect | sets | what |
|---:|---|---|---|
| 1 | x 478 … 510 | `g_screenId = 0x17` | raise an army, after `Levy_SetPercent` |
| 2 | x 512 … 542 | `= 0x09` | the court |
| 3 | x 544 … 574 | `= 0x18` | send supplies |
| 4 | x 576 … 606 | `= 0x1B` | castle building |
| 5 | x 608 … 638 | `= 0x0B` | the other lords |

`docs/screens-county.md` §2.4 had the table and named the last two only by the address they
dispatch to; both are read now. All five are `screens::shells` entries, so all five are
wired.

The same click sweep carries two more controls the sidebar was swallowing: the
**farm/industry labour split slider** (`FUN_00439122`, x 478 … 639, y 257 … 296), which is
the one control on the campaign screen that moves peasants in bulk and was reported as *"I
can't assign peasants"*; and the **four minimap mode buttons** (`g_minimapModeButtons`,
`0x004DC620`), of which the fourth is the zoom toggle `docs/screens.md` §7 records us having
replaced with a key.

**What made it findable.** A player said the icons in the bottom right did nothing and had
text over them. The comment claiming they were a status line had been in the file since the
sidebar was written, and it was never checked against the hotspot table sitting three
sections away in a document this repository already had.

**C44 — The custom game's twelve drop-downs do not write `g_optDifficulty` and its
neighbours. They write a different block, and one function stands between the two.**

The setup screen's twelve options had never reached a game — every one was local screen
state — and the obvious repair was twelve assignments into the `g_opt*` globals
`l2-formats` already imports. That would have been wrong for **seven of the twelve**, and
wrong in a way nothing would have caught for forty turns.

Three functions, all **[V]**:

| | | |
|---|---|---|
| `Setup_SetOption` | `0x00433BA2` | twelve arms, each writing one global in `0x0053F288 … 0x0053F2B4` — a **second** block, a hundred bytes above `g_optDifficulty` |
| `Setup_DefaultOptions` | `0x004AE539` | writes all twelve at once |
| `Setup_CommitOptions` | `0x00499DC3` | runs at *Start*, and turns those twelve into the eleven values a game is played with |

Only five of the twelve are direct copies. One is arithmetic —
`g_aiLordCount = (Nobles + 2) - humanPlayers` — and **five go through a lookup table**:
`g_timeLimitSeconds` (`0x004DBBF8`), `g_startingGold` (`0x004DBC18`), `g_startArmoury`
(`0x004DC070`), `g_startTroops` (`0x004DC110`) and `g_countyStatus` (`0x004DC0D0`). A
*Time limit* of *"4 mins"* is index 3 and **240**; wiring the index to `g_optTimeLimit`
would have produced a three-second turn.

Three things fell out of reading it that were not the point:

1. **The *Defaults* button is not twelve zeroes.** It is `[0, 0, 3, 0, 0, 0, 3, 2, 2, 1, 6, 1]`
   — five lords, a keep, some weapons, a thousand crowns, a medium county — and this tree's
   *Defaults* wrote zeroes, which is a game the original never offers.
2. **`docs/kingdom.md` §8.5 and `l2_game::victory` name the wrong campaign column.**
   Column `+0x0C` was marked **[D]** *"the opponent count"* because it runs 1, 1, 2, 3, 4, 4,
   5, 5. `Setup_CommitOptions` assigns that global from the *Starting Castle* drop-down, so
   the column is the **castle**, climbing wooden → royal up the ladder; the opponent count is
   `+0x1C`, which runs 1, 2, 3, 4, 4, 4, 4, 4. Both are corrected, and the hypothesis
   `g_startForcesSetting` is promoted to `g_startWeapons`.
3. **`g_optFightHumansOnly` is in no saved block, and the original therefore cannot reload
   it.** Adding `0x0053F284` to `l2-formats`' global list turned the battle fixtures red
   with `NotSaved`; the block table covers seven four-byte entries in that range and this is
   not one of them. `docs/bugs.md` has it. The old comment in `l2-scenario` said the option
   was merely *not exposed*; it is not *saved*, which is a different and worse thing.

The lesson is C25's and C29's again in a third shape: **a global's name tells you what
somebody thought it was, and the function that writes it tells you what it is.** Every one
of the twelve had a plausible destination one block away.

**C45 — The shell table called screen `0x17` "Hire mercenaries". It is the raise-army screen,
it is the only door to `Army_Create` a player has, and there is no mercenaries screen in the
game at all. The name set the priority.**

`docs/symbols.json` has called `0x00418653` **`Screen_RaiseArmy`** since it was read, with the
comment *"the levy slider, the six weapon stocks from realm `+0x140`, the happiness cost, and
the mercenary offer for county `+0x1AD`"*. `crates/l2-game/src/screens/shells.rs` called the
same screen **"Hire mercenaries"**, and `docs/armies.md` §5.3 says plainly that the offer
appears *on the raise-army screen*, **not on a screen of its own**.

Two names for one address, and the wrong one was the one an agent picking work would read.
`docs/plan.md` §2.2 diagnosed it before it was fixed: *"a game started from the fixture has no
army, no way to make one, and the door to making one is filed under a name that reads as
optional content"*. Mercenaries are optional content. Raising an army is the game.

This is the fifth correction of the same shape — C25 (bit `0x40` is the county town, not the
castle), C28, C29 and C41 — and the first where the cost was **not** a wrong belief about a
rule.
Nothing anybody wrote about `0x17` was false: the shell drew the right window, the right
`L2.eng` group and the right heading, and its `unfinished` line said exactly what was missing.
The cost was priority. A shell called *"the levy slider, the six weapon stocks and the
mercenary offer"* was a small piece of a subsystem nobody had started; the same shell called
*"you cannot raise an army"* is the last large gap between the engine and the goal, and it sat
in the table for weeks with the other thirteen.

So the class has an extra clause now. **A name is a claim, and a name is also an estimate.**
Correcting one of these has always meant re-reading the code; correcting this one meant
re-reading the *plan*. Where a name understates what a thing is for, nothing downstream is
wrong — it is simply never picked up.

Two things changed with it, both of which the name had been hiding:

* **`engagement::run_siege_phase` was turn phase 2 end to end and nothing called it.**
  `turn::settled` answered the phase-2 wait `true` with the comment *"sieges are out of scope"*
  — true when it was written, and stale from the moment the siege branch landed. A besieging
  army in a played turn built no engines and never assaulted. Same shape as C40: a thing
  everybody described and nobody executed, standing because no test had ever put a besieger in
  a *played turn* rather than in the phase's own harness.
* **`Army_Split` (`0x00437FD7`) charges both halves five movement points**, which nothing had
  recorded. The shipped `Readme.txt` states it in words — *"Splitting does not use all the
  movement for a turn, but cannot be done if the army has used any movement points that
  turn"* — and both halves of that sentence are in the code: the gate is `movesUsed < 1` in
  `Panel_SplitButton` and the *"does not use all"* is `movesUsed += 5` on the parent and the
  daughter. The Readme also states the two rules the code alone reads as arbitrary locals: a
  split *into* a castle has **no** fifty-man minimum and is capped by the garrison's remaining
  room, and a disband goes to the county of origin or, if that has changed hands, to whatever
  friendly county the army is standing in.

**C46 — The corner of every panel was called a tick by five documents and four source
files. It is a cursor arrow pointing into a black hole, and nobody had decoded the frame.**

The player again, looking at the real screen: *"The corner is a hotspot, it's a picture of a
pointer icon, not a mouse. Like it's an arrow pointing to a little black hole. It's a close
button… a weird one"* — and, a minute later, *"i mean it is an instruction"*.

Both. **It is a hotspot whose artwork depicts the action it performs**, and that is the
resolution rather than a contradiction.

`Ui_OkButton` (`0x0040D1BC`) draws `System.pl8` frame `0x33` for mode 0 and `0x10` for mode
1, both 24 × 24 — a fact this repository had recorded correctly since the button sheet was
first read. What it had never done is *look at the frames*. Decoded against `Base01.256`,
both hold the same drawing, mode 1 adding a raised bevel. There is no tick in `System.pl8`
at that index or near it.

The picture is a measurement rather than an impression, which is what makes the entry
**[V]** rather than a second opinion: those two frames are the **4th and 5th darkest of the
sheet's 84**, 15.3% and 11.5% of their area in near-black ink against a median frame's 1.2%,
while the widgets that really are thin strokes sit at 0.2% and 0.0%. A tick cannot come 4th
out of 84. The rank is asserted in `crates/l2-view/tests/install.rs` against the user's own
file, so the label cannot drift back.

It is not a decoration either. `Ui_OkButtonClicked` (`0x0040E7E4`) hit-tests a 24 × 24 box
at `(DAT_0055CE78, DAT_0057C8A0)` on a left release, and `Ui_OkButton` itself writes those
two globals — so the panel has two ways out and the game offers both. One consequence falls
out of the stash being a single pair of globals rather than a table: a screen that draws two
of these makes only the **last** one clickable.

**Both readings that preceded the artwork were half right, and that is the part worth
keeping.** The standing one had a *button* with the wrong *picture*. The reading that
replaced it went the other way: from `L2.eng` group 12 index 0, *"Click Right to Exit"*, and
from `Screen_FrameInput` closing these panels on the right button, it inferred that the
corner must be **signage** rather than a target — a clean story, arrived at from real
evidence, that the artwork does not support. Each was a confident account of one half built
entirely from evidence about the other half. **That is C3's shape inside a single 24 × 24
frame**, and it is the sixth of this species logged today; the fix in both directions was
the same and cost a minute, which was to decode the frame and look at it.

The picture is C43's own finding drawn rather than written. `Screen_FrameInput` closes these
panels on a right release; `L2.eng` group 12 index 0 is *"Click Right to Exit"*; and the
corner is the same sentence as a 24 × 24 icon. Three independent statements of one gesture,
and we had turned the third into a confirm button.

**Two more frames went the same way.** `g_confirmWidgets`' pair, 29 and 31, was written down
as *"a tick and a cross"* in two places. They are 32 × 32 pictures of **a mailed hand with
its thumb up and its thumb down** — which is a medieval game's yes and no, and reads
immediately once seen.

**What made it findable, and the rule it suggests.** A frame index is a *measurement* and
survives; the word beside it is a *guess* and does not. Every one of these labels was
written by someone who knew exactly which frame the function drew and then described it
from the function's name — `Ui_OkButton` draws the OK button, so the OK button is a tick.
That is C28's failure with a picture instead of a name, and it is cheap to avoid: the
frames are in a file we can decode, and looking at one costs a minute.

The symbol keeps its name. `Ui_OkButton` and `l2-view`'s `system::OK` are in half the
interface and renaming them would cost more than the word "OK" misleads; the comment beside
each now says what the frame holds. `docs/screens-county.md` §2.7.

**The player has now been right five times in one day** — the village being an inset (C22),
the county town, the sidebar icons, right-click, and this. Four of the five were things a
screenshot or a decoded frame would have settled at any point in the last year. It is worth
saying plainly: **on questions about the interface, somebody who has played the game is a
better oracle than the decompiler**, because the decompiler tells you what is drawn and he
tells you what it looks like.

**C47 — A new army was put on the county's lowest-numbered road tile. The original puts it
within three tiles of the county's centre, and never looks at the county at all.**

The player: *"I raised an army and nothing appeared on the map."* Two independent faults put
it out of shot and this is the first of them.

`crates/l2-kingdom/src/levy.rs`'s `muster_tile` scanned all 4,096 tiles row-major for the
first free road tile **whose county id matched**, and called that `County_FindFreeRoadTile`.
The real one (`0x00428007`) is four lines:

```c
County_FindFreeRoadTile(county):
    for r in 1..=3: if Map_FindFreeRoadTileNear(county.anchorX, county.anchorY, r) return 1;
    return 0;
```

and `Map_FindFreeRoadTileNear` (`0x0046CFBD`) scans the `(2r+1)²` box around that point,
clipped to the map, row-major, accepting the first tile with `tile.unit == 0` and plane-0 bit
`0x01`. `County_FindFreeOpenTile` (`0x00428078`) is the same walk with `(flags & 0xFD) == 0`.
**[D]**, and the two functions are byte-for-byte the same shape.

Three facts follow, and all three were wrong here:

* **The search is bounded at radius 3.** A county is tens of tiles across and the near view
  is eight lattice columns wide, so our tile was routinely off the side of the screen. On the
  England fixture the player's county 8 has its anchor at (22, 36) and our muster tile was
  (30, 33) — fourteen lattice columns from the viewport's left edge. The original's is
  (23, 35), one tile from the anchor.
* **The county is never tested.** A county whose anchor sits near a border can raise its army
  onto a neighbour's tile, and the original allows it.
* **The fallback is not "passable".** `& 0xFD == 0` admits bare ground and the county-boundary
  bit and nothing else — not farmland, not rough ground, not a settlement, not a road. The
  road pass is the only way an army lands on a road, and a county boxed in by its own fields
  has nowhere to stand.

`Unit_Spawn`'s own precondition, `(tile.flags & 0xFC) == 0`, is stricter still than the road
finder's and is now reproduced: a road tile that also carries farmland or a settlement passes
the finder and fails the spawn, and `Army_Create` returns 0 — message `0xDD`, the same
refusal as nowhere to stand.

**The shape.** The name was right, the address was right, and the body was written from the
name. That is C28 again, and what exposed it was not a re-reading but a player looking at his
own screen.

**C48 — The campaign map opened at `Map_InitMode`'s scroll origin and stopped there. The
original centres it on the player's own town before the first frame.**

The second half of *"nothing appeared on the map"*, and it is why the player also could not
find his merchants or his county.

`docs/screens.md` §1.5 records `Map_InitMode`'s opening state — near zoom, row `0x4A`, column
`0x14` — and `crates/l2-game` reproduced it exactly. It is not the whole of the original's
bring-up. The last two statements of `Game_SetupRealmsAndCounties` (`0x0049BD99`), after every
realm has its county, its gold and its starting garrison, are

```c
FUN_00432746(g_playerStartTable[g_localPlayer * 2]);
FUN_0046dfd5(g_playerStartTable[g_localPlayer * 2]);
```

and `FUN_00432746` (`0x00432746`) is

```c
DAT_0053f0dc = g_counties[county].townTile;
if (DAT_0053f0dc != 0) { Map_CentreOnTile(DAT_0053f0dc); g_selectedCounty = county; }
```

So **a new game opens looking at the player's own town**, not at row `0x4A`. Ours opened on a
stretch of England the player owned nothing in: eight lattice columns, and county 8's town
fourteen columns outside them.

Two departures, both deliberate and both at the call site. The original does this at
`Game_NewGame` time; we do it the first time the campaign screen is built, because a screen is
constructed from a `ScreenId` with no game in hand. And it centres on the *start* county from
`g_playerStartTable`, which a loaded position does not carry, so we centre on the selected
county when it is the player's and otherwise on his lowest-numbered one — the same county on
turn one. The zero guard is the original's: a county with no town tile moves nothing, which is
every synthetic map in the test suite.

**One thing this cost, and it is the interesting half.** A lazily-applied default silently
overrode explicit positioning: two existing tests centre the map on a tile and then click it,
and the first `ensure` recentred underneath them. An explicit `centre_on_tile` or a scroll now
counts as "positioned", so the opening centre is a default for a screen nobody has placed
rather than something that happens to every campaign screen once. A default that outranks an
instruction is a bug wherever it appears, and here it took two red tests to say so.

**C49 — `FUN_004081A6` was called `Map_DrawCountyFlag` in four documents and `symbols.json`.
It is the gold path-preview ball. The county flag is in a different function, off a
different bit, in a different plane.**

The player: *"each county's town square would have a coloured flag waving on it, and castles
with armies in them have a flag."* Neither was drawn, and the entry point recorded for both
was the wrong function.

`0x004081A6` draws `Flags1a.pl8` frames `0x38 + cost` on tiles carrying **bank** bit `0x40`
— which `Path_MarkPreviewTiles` (`0x004A91BA`) sets on each step of the local player's
ordered path and `FUN_0046C6BA` clears grid-wide. `docs/armies.md` §2.3 had that right and
called it `Map_DrawPathMarker`; `docs/screens.md` §5, §1.3, `docs/formats/maps-layers.md`
§5.3 and `symbols.json` all carried "the county flag on a castle tile, frame
`castleLevel + 0x38`". Every clause of that is wrong, and the frame arithmetic it quotes is
the path ball's.

The flags are **`FUN_004071A0`**, gated on **bank bit `0x80`**, which
`County_FindTownTile` and `County_FindCastleTile` set on their anchor quadrants. Inside, the
branch is on **plane 0**, and the two `0x40`s are in different bytes: bank `0x40` is the
transient path mark, plane-0 `0x40` is the county town.

```c
if      (flags & 0x40)  /* the town   */ { part 0: shield = county.shieldIndex;   /* +0x07 */
                                           part 2: mercenaryOffer ? frame 0x81 : return; }
else if (flags & 0x80)  /* the castle */ { if (content == 0x14) return;           /* no castle */
                                           if (!county.garrisonUnit) return;      /* +0x1BC   */
                                           shield = units[county.garrisonUnit].shield; }
frame = shield * 8 - 8 + phase;
```

**The colour is the frame index.** `Flags1a.pl8`'s first forty frames are 32 × 24 and lie on
the artist's sheet as five rows of eight — **five shields × eight wave phases** — and
`shield = 5, phase = 7` lands on frame 39, the last of them, with frame 40 starting an
unrelated block. There is no palette remap. **[V]**, and the arithmetic closing on the frame
count is what makes it verified rather than plausible.

Three consequences worth keeping:

* **The castle's flag carries the *garrison's* shield, not the county's**, so a captured
  castle still holding somebody else's garrison flies their colours and a county's two flags
  can disagree.
* **`content == 0x14` returns.** `0x14` is the bare castle plot and `0x15 … 0x19` are castle
  types 1 … 5 (`FUN_0046826C` stamps `0x14 + castleType`), so an unbuilt castle flies nothing
  however large its garrison. Our `industry::map_toggle_for_graphic` already draws that line
  in the same place — 13 … 20 is nothing, 21 and up is the castle — which is an independent
  confirmation from a ladder read out of `Map_Click`.
* **The town's 2 × 2 carries two markers**, on plane-3 quadrants 0 and 2: the owner's flag
  and, when the county has a band standing, `Flags1a.pl8` frame `0x81` — the mercenary offer,
  advertised on the map.

**The wave is a global counter, not a per-tile phase.** `FUN_004CFB08` advances
`DAT_0057D378` once per **16 ms** of `GetTickCount` and then draws a frame; `Map_DrawFrame`
wraps it at `0x80` and sets `DAT_0057D390 = tick >> 4`. Eight frames, 256 ms apiece, a
2.05-second wave, and every flag on the map is in step. Our fixed tick is 16 ms already, so
the counter is stepped by `Screen::update` and the arithmetic is carried across unchanged —
and only a *change of phase* asks for a repaint, so a still map with flags on it costs eight
frames every two seconds rather than sixty a second.

**Why it went unnoticed for so long.** The name was in `symbols.json` with a `[verified]`
comment, and four documents cited the comment rather than the function. The bit was right
(`0x40`), the sheet was right (`Flags1a.pl8`), the clip rectangle quoted from it in §1.3 was
right — everything checkable at a glance agreed, and the one thing nobody checked was what
the frames were for. C46's rule again, one level up: **a frame index is a measurement and
the word beside it is a guess**, and here the guess had propagated into four documents
before anything looked at the artwork.

**C50 — "your merchant". There is no such thing: every merchant in the game is ownerless,
and the guard is whose *county* it is standing in.**

The player: *"I don't see the merchants on the map and of course I can't click them."* Both
halves were true, and both came from the same wrong idea.

`docs/screens.md` §6 and `symbols.json`'s `Map_Click` entry both say *"your merchant (unit
type 3) opens screen `0x08`"*. `Map_Click` (`0x0043CE1A`) reads a **unit's** owner byte
exactly once in its 1,263 bytes, on the `kind == 1` path, and never on the merchant path.
The merchant arm's guard is `g_counties[g_pickedTileCounty].owner == g_localPlayer`, and it
additionally does nothing at all — not even the refusal message — when that county has no
town tile.

That matters because **`Merchant_SpawnAll` (`0x00427ED0`) passes 6 to `Unit_Spawn`
unconditionally**, 6 being `Army_Create`'s ownerless marker, and nothing ever rewrites it:
all six merchants of the England turn-one fixture and all three of the siege fixtures carry
owner 6. A guard on the merchant's owner could never have fired. So the sentence was not
merely imprecise — it described a control path that does not exist, and our engine
implemented it faithfully: `click_unit` answered `NOT YOUR UNIT` for every merchant on the
map, for ever.

**A merchant belongs to nobody. It is county infrastructure**, and five independent things
say so: the spawn, the saves, the three functions that never read its owner,
`County_RecountMerchants` (`0x00451061`) filing merchants under a *county* with no realm
touched, and `L2.eng` 31/12 in the game's own words — *"Merchants allow a county to buy
needed supplies and raise revenue by selling goods."*

**The other half — not seeing them — was ours, and it was the same mistake wearing a
different hat.** Units were drawn as squares in `Ink::realm[owner]`, and `Ink::realm` has six
entries, `0 ..= 5`. Owner **6** fell off the end into `ink.dim`, so every merchant in the
game was a small beige square on beige-and-green terrain. The fix is not a brighter square:
`Map_DrawArmies` (`0x00408438`) draws merchants from **`Sprite1a.pl8`, the same sheet as the
armies** — only a transport uses sheet B — and its frames 0 … 47 are a 40 × 32 merchant, 8
facings × 6 walk frames. Those are placed now, and so are the armies' three size banks.

Two arithmetic corrections fall out, both against `docs/screens.md` §5:

* the rotation is **`(facing + 1) & 7`**, not `facing`, in all four tick handlers;
* the sheet is chosen by **`kind == 4` alone**, not "armies use `sprite1a`/`sprite1b`".

`0x08` is still a shell in this tree: the route is real, it centres the map on the town the
way the original does, and it lands on a screen that draws `Merchant.pl8` and does not trade.
That is deliberate. `DAT_00553C64`, which the original sets on this line, has exactly one
writer in the whole binary — this line — and two readers, both in the merchant screen's price
arithmetic, so when `0x08` grows a trade this is the call that has to carry the unit into it.

**C51 — The sound banks were read 0-based. They are 1-based, and every consequence of the
wrong reading was plausible.**

`Sound_LoadBank` (`0x00425E4B`) writes the loaded buffers to `&DAT_00522B00 + i * 4`,
counting `i` from 0. `Sound_PlaySlot` (`0x00426120`) and `Sound_RestartSlot`
(`0x00426216`) read from **`&DAT_00522AFC + slot * 4`** — a different base, four bytes
lower. So `slot n` is bank entry `n − 1`, and the two functions never appear in the same
screenful of decompiler output.

Read 0-based, everything still *worked*. The campaign bank's twelve names came straight
out of `.data`, `Unit_MoveInFacing` plays slot 12 for an army, and the resulting story was
"slot 12 is past the end of a twelve-entry bank" — an odd fact, not an impossible one, and
one that reads as a quirk of the original rather than an error in the reading. Every other
consequence went the same way: `Unit_MoveInFacing` gave a peasant mob `Fallow.wav` and a
merchant `Army.wav`; the village job table gave grain `Stonecut.wav`, cattle
`Rioters.wav` and fields `Wheat.wav`; and slot 0 was never passed by anything, so
`Click3.wav` looked like a sound with no trigger and was written into `docs/bugs.md` as
dead content. **Nine wrong facts, none of them absurd on its own.**

**What caught it was a second agent's independent finding**, arrived at from the campaign
map rather than from the sound tables: *"`Army.wav` for kind 1, `Merchant.wav` for kinds 3
and 4, `Rioters.wav` for kind 2"*. Two readings of the same three lines disagreed, which
is the only reason anyone went back to the base addresses.

**The rule this suggests is about the shape of the check, not about being careful.** The
disproof was sitting in the data the whole time and cost thirty seconds once looked for:
`g_jobSound` maps six village jobs to six slots, and 1-based they are wheat, cattle,
fallow, iron, stone and wood — **six for six**, which cannot happen by chance. 0-based
they are stonecut for grain and rioters for cattle, which is noise. **A table that maps
one named thing to another named thing is a self-checking oracle**, and this project has
one for nearly every index it reads. The check to reach for is not "is my arithmetic
right" but "does my arithmetic make the names line up" — and where it does not, the
arithmetic is wrong even when it looks fine.

That is C28's failure with an index instead of a name and C5's with a table instead of a
format: a confident account built from real evidence about the adjacent thing. It is the
seventh of the species logged, and the cheapest to have avoided.

`crates/l2-game/src/audio/names.rs` now converts in exactly one place, `names::slot`, with
both proofs as tests; `docs/mechanics.md` carries the corrected tables.

**C52 — The blue outline is real, it is a *frame*, and it is on the county strip. The
player remembered it in two places and was right in both; this document has now been
wrong about which job it tests.**

Five interface defects reported in one message, four of them about assigning peasants:
*"The peasant slider of industry isn't draggable, should be. I can't double click idle
peasants in a task to remove them from the task, there's no 'blue outline' for idle peasants
(eg too many on dairy)… The text should be black not white over the happiness, the peasant
slider I think had a blue outline if there were idle peasants as well."* All five are in the
binary.

**The blue outline is `Misc_cty.pl8` frames `0x4B` … `0x55`, and it is measurable.** Eleven
frames, a contiguous run, and every one of them is drawn two pixels up and left of the plain
frame it replaces. Ten of the eleven are **exactly four pixels wider and four taller** than
that plain frame, which is what a two-pixel ring around an unchanged picture measures as,
and **every non-transparent pixel of that two-pixel border is one of three palette entries,
all of them blue**: `95` = `rgb(0,0,121)`, `65` = `rgb(157,202,234)`, `64` =
`rgb(194,230,255)`. That is asserted in `crates/l2-view/tests/install.rs` against the user's
own file, so "blue outline" is now a measurement rather than a recollection. The eleventh —
the castle, `0x40` → `0x4E` — is 23 × 26 against 32 × 34 and is a different, larger picture
that also carries the ring; it is written down as the exception rather than smoothed over.

**Two signals, and they are *not* the same test**, which is the part the player's memory
merged and the part this entry exists to separate:

| where | condition | frames |
|---|---|---|
| the farm/industry slider's thumb | `county.labour[8].workers != 0` — **anybody** idle | `0x3D` → `0x55` |
| a produce icon on the strip | `labour[slot].useful < labour[slot].workers` — too many on **that** job | five pairs |

`crates/l2-game`'s `county.rs` carried a comment saying the swap happened *"when the castle
job has workers"* and that *"which of the nine job slots that test reads is not settled"*.
It is slot **8**, idle townsfolk, and it is one line of `CountyStrip_Draw`. The guess had
been sitting beside the answer.

**The outline is the other end of a record we already had.** The labour record is three
`i32` per job — workers, a wanted floor, a useful ceiling — and `Panel_JobDetail` already
coloured the count red *below* the floor. The blue ring is *above* the ceiling. Same three
words, both ends, and neither was invented for the occasion.

**The double click is its own input arm, and the single click is deferred to make room for
it.** `Village_DoubleClick` (`0x00439DF0`) sits between `Village_BandStart` and
`Village_ClickJob` in `Screen_FrameInput`'s screen-`0x02` ladder and is the **only** reader
of `DAT_004EABC5` in the binary — a flag the window procedure sets from message `0x203`,
`WM_LBUTTONDBLCLK`. So Windows counts the clicks, not the game. `Village_ClickJob` is then
gated on `DAT_004EABF0`, which the frame poll sets only once a click has stood **300 ms**
without a second one: the job popup deliberately opens a fraction late, because until then
the click might be half of a double click. Ours does the same, counted in ticks rather than
milliseconds, because nothing below `main.rs` may read a clock.

What the gesture *does* is `FUN_00439F6A`: a job below its floor takes people from the idle
pool, a job above its ceiling puts its surplus back, and a double click on the idle cluster
runs both passes over every job — shedding first, filling second, because one pass would let
whichever job came first take people the later ones needed.

**The slider is press-and-track, not the village's three-screen gesture.** `FUN_00439122`
acts while `DAT_004E65CC` — the button's *level* — is set and `DAT_004EA4B0` says the
pointer moved, and returns without doing anything on the release. No dead zone, no second
click, and `g_screenId` is never touched. Reading the three flags out of the frame poll at
`0x004B2D5A` settled in one sitting a question that would otherwise have been a guess
between two plausible mechanisms.

**And the strip's text is `0x3F`, which is black.** Every `Ui_DrawText` call in
`CountyStrip_Draw` passes it — the county's name, the population, the happiness, the tax
rate, both captions — and `0x3F` in `Base01.256` is `rgb(0,0,0)`. Ours drew all of them in
`Ink::text`, which resolves to white, on the original's parchment plate. C42 had already
corrected the *anchoring* of the same two calls by reading their arguments; the colour
argument was the last one on the line and nobody read it either. **The argument was there
both times.**

**The player is now right ten times out of ten on the interface**, and five of those were in
a single message. The pattern from C46 holds exactly: every one of these was a *word* beside
a measurement — "castle job" beside a slot number, "tick" beside a frame index, `ink.text`
beside `0x3F` — and in every case the measurement was already in the file.

**C53 — Ale's five happiness are a *seasonal* allowance, not a lifetime one. Four documents
said "nothing in the binary resets it", and the reset is eleven lines away from a function
all four of them cite.**

`Ale_Apply` (`0x00428C42`) caps its gain at `5 - county[+0x219]`, the happiness ale has
already given this county. `docs/kingdom.md` §7.6, `docs/mechanics.md`,
`docs/symbols.json`'s `Ale_Apply` entry and `l2_kingdom::county::County`'s own doc comment
all drew the same conclusion from the same search: *nothing writes `+0x219` except
`Ale_Apply`, so the five points are for the life of the game and a county that has had them
will never gain from ale again.*

`Happiness_UpdateAll` (`0x0044BAEA`) writes it, every season, for every county:

```c
g_counties[i].shownArmy   = 0;
g_counties[i].shownEvents = 0;
g_counties[i].aleHappinessGiven = 0;      /* +0x219 */
g_counties[i].shownAle    = 0;
```

It sits **between two fields all four documents had already described**, in the middle of the
display-field reset every one of them quotes. The search that missed it was for writers of
`+0x219` in the *ale* code; the write is in the *happiness* code, one line above a field
(`shownAle`) that our own crate was already resetting correctly on that very line. The
crate reset the display and left the allowance, so our ale was strictly meaner than the
original's for the whole game.

**`Readme.txt` had said so, and it was read as being about something else.** Its *"Ale
Limitations"* section — *"Ale can only be purchased once per county per season. Its benefit
is also limited to +3 happiness per purchase"* — was on file and cited for the *purchase*
limit. Its plain claim that the benefit is **per season** is the correction, and it went
past because the sentence people quoted from it was the other one. The errata is a
first-class oracle (`CLAUDE.md`), and here it was right about the horizon and this binary
disagrees with it about the number: the ladder's cap is **5**, as `MOV` immediates in
`Ale_Apply` and again in `Ale_PreviewGain`, not 3. Both are recorded rather than reconciled,
because "the Royal Edition readme describes a +3 that this executable does not implement" is
a finding and averaging them would not be.

The shape is C42's and C52's for the third time: **the fact was inside a function the
document already cited, on a line beside one it already quoted.** What is different is that
here a *fifth* source — the game's own errata — had stated the conclusion in English, and
the search that would have found it was a search of the same file for a different phrase.

**C54 — The four merchant accumulators are not "this season" and "the total". Nothing
resets them and nothing reads them, and the hypothesis said so itself.**

`docs/hypotheses.json` named realm `+0x104`/`+0x108` `spentThisSeason`/`spentTotal` and
`+0x10C`/`+0x110` `receivedThisSeason`/`receivedTotal`, with the caveat *"nothing in the
corpus resets either, and if neither is ever reset the pair is something else entirely."*
The check the caveat asks for now runs: across the whole binary the only writers are
`Merchant_Trade`, which adds to both of a pair, and `Game_SetupRealmsAndCounties`, which
zeroes all four at new game. **There are no readers at all.**

So the subject half of each name is verified — two accumulate purchases, two accumulate
sales — and the role half is refuted. They are promoted to `docs/symbols.json` as
`trade_spent_a`/`_b` and `trade_received_a`/`_b`, deliberately unhelpful names, because a
name that says *when* would be a claim about a horizon this executable does not have. The
entries are deleted from `hypotheses.json` rather than softened, per that file's own rule.

The general point is worth keeping: **an unread accumulator is evidence about a feature that
was cut, not about one that exists.** `g_goodsStock` at `0x004D8950` is the same finding on
the other side of the same screen — fifteen entries that read exactly like a merchant's
stock, and no instruction anywhere reads them. Two cut mechanics, both visible only as data
nobody consumes, both on the merchant.

**C55 — A missile is not a hit that is animated. It is an object that traverses, and it
strikes whoever is standing in the cell it enters — not whoever it was aimed at.**

`docs/mechanics.md` carried *"missiles are computed and never fired — a hit resolves,
nothing flies"* for a long time, and the cost of it was measured the moment the
campaign–battle seam landed: sixty archers fought as sixty men carrying unused bows, and the
fought path disagreed with the fixture oracle while the autocalc reproduced it to the man.
The seam test therefore asserted that the books balanced and **deliberately not the winner**.

**The question that had to be settled before a line could be written** was whether the
original resolves a shot at launch and merely animates it, or whether the arrow genuinely
traverses and can be blocked. It changes the answer completely: the first is a distance
check and a damage roll, the second is a hundred-slot array of objects in the lockstep
state. It is the second, and the evidence is one line of `Missile_Step`:

```c
g_otherBattleMan = g_battlefield[missile.cellOffset].figure;
```

The victim is read out of the cell the missile has **just entered**, fresh, every sub-step.
**Nothing anywhere in the 0x4C-byte record remembers who the shot was aimed at** — `+0x06`
is the *shooter* — so an arrow cannot check whether it hit the right man, and does not. A
body in the flight path takes it; a miss coasts on past the target and can kill a second
rank; a target that dies or walks away is not tracked; and the friend/foe test is on the
**owner** byte rather than the side, so a friendly body neither takes the arrow nor stops it.

Three more facts that were not guessable and are now `[V]`. **The timing is one number
wearing two hats**: four sub-steps a tick at 1/32 of a cell each is exactly ⅛ of a cell a
tick, and the range in `g_missileStats` is stored in *eighths of a cell* — so the same
number is the distance and the tick budget, and `range >> 3` is the range in cells with no
conversion anywhere. **A missile is born a cell out**: `BattleMan_FireMissile` runs eight
`Missile_Step`s on the spot before the arrow is ever drawn, and a shot can already have hit
something before anybody sees it. And **one hit per missile is structural rather than a
rule**: every impact test is gated on `ttl == 0`, and a hit sets `ttl = 2`.

**The fixture is the check, and it moved.** The same saved position, the same seed, the same
eleven counts a side: the fought battle used to be **won by the player with 56 men of 178**
against a saved game in which he lost and both armies were destroyed. It is now lost by him,
the militia holding the field with 8 men of 182 where the autocalc's ladder walks 36 home.
`crates/l2-game/tests/seam.rs` asserts the verdict now, and the caveat is gone with the gap
it described.

**Two things the brief and this tree had wrong, found on the way.** `Missile_Step` does not
raise `g_siegeBreachScore`; it adds **one** to a wall cell's own counter, and only the
sixteenth hit collapses the cell — `FUN_0047DFE0`, which is what scores, one point per
orthogonal neighbour still standing. And missile **class 7 is boiling oil, not a falling
man**: `docs/battle.md` §0 and `docs/symbols.json` both said *"falling men"*, the only
spawner in the binary is the oil path, and there is no class 6 at all.

**C56 — The completeness check for the *battle* checksum did not exist, and writing one
found `Fighter::progress` outside it.**

C39 established the rule — *completeness must be derived, not remembered* — and derived a
check over `l2-kingdom`'s save. Nothing did the same for `l2-sim`, whose lockstep encoder is
a hand-written list in `crates/l2-sim/tests/lockstep.rs`, and a hand-written list cannot fail
for a field nobody put in it.

Adding the missile array was the occasion to write one. It reads the field names of `Missile`
and `Fighter` out of the source and requires each to appear in the encoder; its first run
failed on **`Fighter::progress`**, the sub-cell walk counter. Two peers that disagreed about
how far a figure was through its current cell would commit to the next cell on different
ticks — and every checksum they exchanged up to that point would have agreed. Exactly C39's
shape, one crate over, found the moment the check was derived rather than remembered.

The exemption table has C39's polarity: inclusion is the default, a line is a claim with a
reason attached, and a line naming a field that no longer exists fails too. The missile array
is hashed **slot by slot over all hundred**, live and free alike, which is C39's *second*
property — a whole record can go missing from a sweep and no amount of field checking sees
it.
**C57 — The county's four industry records were never imported, so every county claimed
every resource; the village drew none of the three buildings that depend on them; and half
of the mine on the map was not clickable because the building overhangs its own tile.**

A player reported *"a county that clearly has iron has no iron mine in the town center and I
can't click the iron mine on the world map to enable/disable that."* It reads as one defect
and it is three, on three different layers, and only the middle one had ever been looked at.

**1. `+0x295`, `+0x296` and `+0x297` were not read.** `l2-scenario` imported fifty-odd county
fields and skipped the four twenty-four-byte industry records entirely, so every county
arrived from `County::new()` with `has_resource: true` and `enabled: true` — all four
resources, all four switches on, in all fourteen counties. The file says otherwise, and it
**checks itself against the map**: `County_PlaceResourceSites` (`0x00468E61`) writes `+0x295`
at load from the county's `Town`-bank tiles, so the byte and the terrain are two recordings
of one fact. Over the England turn-one fixture they agree **56 times out of 56** — and it is
not a vacuous agreement, because 15 of the 56 are *false*: iron and stone come out
complementary in thirteen of the fourteen counties and county 5 has neither.

The switches are just as far from the default: **five of the fifty-six are on**, and they are
the wood cutting of exactly the five counties that start owned. So the map's toggles had all
been drawn — and, worse, *acted on* — in the opposite position to the one the player sees.

**2. `Village_Draw` blits three buildings and we drew none of them.** Read out of the
corpus, the three calls are consecutive and each is gated on a resource:

```c
if (county.industry[0].hasResource) Pl8_DrawFrame(villani2, 0x29, 0xac, top + 0xe5);  /* wood  */
if (county.industry[3].hasResource) Pl8_DrawFrame(villani2, 0x28, 0x4c, top + 0x0c);  /* stone */
if (county.industry[1].hasResource) Pl8_DrawFrame(villani2, 0x2b, 0x4c, top + 0x0c);  /* iron  */
```

Three things that were guessed and are now read. **The sheet is `villani2.pl8`, not
`Misc_cty.pl8`** — `l2-view`'s own doc comment named the right frames in the wrong file, and
`villani1.pl8` and `villani2.pl8` were not loaded by anything. **The lumber camp is a third
building**, at the bottom right; the documented pairing was only the two that share a spot.
And **the mine is drawn after the quarry at the same coordinates**, which is safe precisely
because of the complementarity the fixture shows: no county holds both, so the later blit
never covers the earlier one.

Note the distinction that was already right and stays right: a county with *neither* still
shows cluster 0's **slot** — `Village_DrawPeasants` loops 0 … 7 unconditionally. What was
missing was the *building*, not the slot.

**3. The mine overhangs its tile, and the hit test was the tile.** `Town1a.pl8` frame 30 is
**58 × 47** on a 58 × 30 tile — seventeen rows of headframe above the diamond, and more of
the building inside the diamond's bounding box but outside the rhombus. Swept pixel by pixel:
**1,314 pixels of the mine are painted and 857 of them were on the tile.** The whole upper
half of the building — the part anybody aims at — was dead, and because our map has a
fall-through the original does not have, a click there **opened the county panel** instead of
doing nothing. The forest is worse at 22 rows of overhang.

**This is the one deliberate departure from the binary on that path, and it is stated as
such.** `Map_PickTile` (`0x00429ba4`) is pure geometry: it divides by the pitch and resolves
the diamond with a parity and remainder test, and never looks at a pixel — so in the original
the top of the mine belongs to the tile behind it, where `Map_Click` finds no flags and does
nothing at all. Ours tries the diamond first and, only if that misses, tests the tile's own
frame **opacity mask**. It can add an answer where the original had none; it can never move
one. The alternative — reproducing the dead zone faithfully — reproduces a defect that this
project's own fall-through makes considerably worse than it is in the original.

**Two more readings fell out of `Map_Click` and are recorded rather than acted on.**
`Map_ResolvePick` (`0x0046D5FE`) **snaps a click on a multi-tile object to its north-west
anchor**: `part & 0xf` is divided and remaindered by the object's width to walk back to the
first tile, so all four quadrants of a town or a standing castle resolve to one. And the
whole dispatcher is wrapped in `if (g_mapZoom != 2)` — **at the far zoom the campaign map
does not respond to a left click at all.**

**What the pattern is, since this is the fifth visual defect a person has reported after the
code was merged.** C41's lesson was *a renderer verified against its input file is verified
against the wrong thing*. This one is narrower and sharper: **the village screen had eleven
tests and not one of them asked what the county's own record said.** Every test drove the
picture from the *labour* fields, which were imported, and none from the *industry* fields,
which were not — so a struct field that no importer ever wrote and every screen read as
`true` passed the whole suite. The three assertions added here are all of the same shape:
they compare two independent recordings of one fact (the record against the map, the drawn
region against the frame it should hold, every painted pixel against the toggle it should
reach), and each of them fails on the code as it stood.

**C58 — A unit is picked by its *tile*, not by the marker we drew on it. Ours asked a
nine-pixel box on a 58 × 30 diamond, so most clicks on a merchant missed it — and our own
"a second click opens the county" caught them.**

The player, in the same session as C57: *"there seems to be some weird thing where a certain
county is 'selected', and if I click a merchant while the map has a different county selected
it will open up the tax window."*

**The selection is innocent.** `Map_Click` consults `g_selectedCounty` for exactly one thing,
and it is not a hit test: the site and merchant arms compare it with the picked county in
order to decide whether to *re-centre* first. The pick itself is positional throughout.

**The hit test was the defect, and it is C57's defect again.** `Map_ResolvePick`
(`0x0046D5FE`) reads `g_pickedTileUnit = g_tiles[t].unit` — **the whole tile is the unit**.
Ours hit-tested the little square marker instead, `unit_marker_half + 1` around the tile
centre, which is nine pixels across at the near zoom; the merchant that is actually drawn is
a 40 × 32 figure from `Sprite1a.pl8`. So a click on the visible merchant usually resolved to
no unit at all, fell past the settlement, town and field arms, and landed on ours. Fixed the
same way and for the same reason: **the tile first, which is the original's entire answer**,
then the drawn figure's opacity mask, because `Map_DrawArmies` anchors a sprite on the tile's
*bottom vertex* and it therefore stands up over the tiles behind it. The tile wins whenever
it holds a unit, so the fallback can add an answer and can never move one.

**What a click on empty ground does in the original: nothing.** This was asked as a separate
question and it has a flat answer — `Map_Click` has **no county-selection arm**. Its only
writes to `g_selectedCounty` are inside the merchant, town and industry-site branches, beside
a `Map_CentreOnTile`. Selecting a county from the map is entirely ours, and so is the second
click that opens its panel.

**That is worth stating on its own, because it is the amplifier under both reports.** In the
original a hit test that misses costs nothing — the click falls off the end of the function
and the player clicks again two pixels lower. In ours a miss *does something*: it selects,
and a second miss opens a modal county panel. Every geometric shortfall anywhere on the map
is therefore converted into a visible wrong screen. The convenience is kept for now — the
county strip needs a selection and the original's routes to one are the strip and the minimap
— but it should be read as a standing multiplier on hit-test accuracy rather than as a free
extra.

**C59 — The flags and the minimap tint were not broken. The sidebar being dead under the
village was, and the reason a human found all three is that the suite never looked at the
canvas.**

A player reported two things missing from the campaign map: *"I noticed the minimap had
default colors, it didn't actually identify who owned a county, and I didn't see the colorful
waving flag over my county."* **Both render at `main`, on his own save as well as the
fixture**, and the way to know that is to render rather than to read the code.

| claim | measured at `main` |
|---|---|
| the town flag | `Flags1a.pl8` frame `(shield−1)×8+phase` matches **exactly** on the canvas: **285 opaque palette indices** of a 32 × 24 frame, all agreeing, at (236, 176) — on the England fixture *and* on the install's own `lastturn.sav` |
| the minimap tint | setting every county unowned moves **2,651 pixels** inside the 116 × 116 rectangle, and the ramp rows those pixels came from are **{1, 2, 3, 4, 5}** — exactly the five colours England's five owning realms fly |

**So what was he running?** The flags reached `main` in `098895e`, at **01:17 on 9 September**,
three commits before the report; the minimap's owner tint reached it in `c67d0e8` at **00:51 on
8 September** and has not changed a line since. There is no build in which both were absent
*and* the sidebar he had just filed five defects against existed. The flag half is simply a
binary built before 01:17; the minimap half does not correspond to any build, and the most
likely reading is that he saw the untinted **unowned** counties — row 0 is the raster's own
shading, deliberately, so on England nine of the fourteen counties really are "default
colours" — and generalised. Worth knowing: *"the tint is missing"* and *"most of the map is
unowned"* look identical to somebody who has not counted.

**The pattern is real even though these two were fine.** Three visual features have now been
reported missing after being merged. The flag and the merchant both *did* have a pixel test —
each diffs two canvases and checks the ink landed inside the right rectangle — and **the
minimap's tint had none at all**. A diff says *something* changed in a box, so it passes a
garbage sprite and it passes the wrong frame of the right sheet. The two tests added here
match the **artwork itself** and then move one field of the save and require the picture to
follow: change `shield_index` and the *same pixel* must fly the other shield's flag; change
who owns the counties and the set of ramp rows that move must be exactly the set of owning
colours. The minimap one has to be a **difference** rather than a census, because `MAPnn.PL8`'s
raster carries pixels the overlay never touches and one of them (`0x38`, the beige) is by
coincidence an entry of ramp row 3 — a census reports a realm nobody owns.

**And the failure mode is worth naming, because it is not blankness.** `chrome::realm_colour`
clamps a zero colour *up* to 1. A tint that has lost its input does not go grey; it goes
uniformly **red**, which is a plausible-looking picture. That is exactly the "a wrong offset
reads as zero and a clamp turns it into a plausible colour" trap `scenario.rs` already refuses
to walk into by storing the colour byte raw — the same trap, one layer up, and it is why the
new test asserts *one row is the shape of the failure* in its own message.

**And writing the test for the *other* flag found something the report had not.** C49 claims
two flags — the town's, in the county owner's colours, and the castle's, **in the garrison's**.
Trying to assert the second one against the five siege fixtures skipped: not one save in the
tree had a garrisoned castle. They all do. **The relation is stored from one end and read from
the other, and nothing joined them.** The original keeps both halves — county `+0x1BC` names
the unit, unit `+0x198` names the county, both **[V]** in `docs/armies.md` — and
`Castle_Garrison` writes them together; `l2-scenario` imported only the unit's half, and
`County::new` seeds `garrison_unit: 0`. So every loaded game arrived with **no castle
garrisoned anywhere**, and the castle flag was not merely unseen by the player, it was
**unreachable**. It is one loop, and the place it goes is not obvious: put it where the units
are installed and the county loop's `*c = County::new()` wipes it a hundred lines later, which
is where it went first.

The flag is what exposed it and the flag is the least of it. `conquest`'s ownership test,
`divide`'s garrison arm and `siege`'s still-inside check all read the same field and had all
been reading zero. `l2-scenario`'s
`a_castle_garrison_reaches_the_county_it_is_standing_in` runs over **every** save the machine
can offer rather than one fixture, because the failure was silent on all of them.

**The real defect was three feet to the right.** The same player: *"The slider on the right is
disabled when the town square window is open, I remember that being clickable I think, I'll
check."* He checked, in the original: *"everything is still clickable with the town square
open… The slider does indeed still work with town square open and causes no issues."* He is
right, and `Screen_FrameInput`'s `g_screenId == 0x02` arm says so outright — six guards before
a single village verb, and all six are the campaign map's:
`FUN_0043292d` is `Hotspot_Test(0x262, 0x20, &g_minimapModeButtons, 4)`, `FUN_00432967` is
`Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)`, then `CountyStrip_Click`,
`Labour_SplitSliderDrag`, `CountyStrip_JobClick`, `FUN_00439079`. Every one hit-tests
`x >= 0x1DE` = 478, so the rule is *"the column at 478 keeps working, and nothing else does"* —
`Map_Click` is **not** in the ladder, so a click on the terrain round the inset still does
nothing.

**The `0x05` and `0x06` arms test none of them.** Banding and carrying run
`Village_BandRelease` / `Village_Drop` and stop. So the sidebar is dead for exactly as long as
a peasant is in the air, which reads as intermittent from the outside and is the kind of thing
that gets tidied into uniformity by somebody who never read the other two arms. It is
reproduced and asserted, `docs/bugs.md` B63a.

**`Transition::Pass` is what it cost, and it brought a second finding with it.** Our machine
offered input to the top screen and stopped, which is right for a modal popup and wrong for an
inset whose arm begins with somebody else's guards. A screen may now decline an event and let
the one underneath have it — and the moment a lower screen can *act*, the question is where
its `Push` lands. The original answers it: **`g_screenId` is one byte and there is no stack**.
Of the 100 writes to it inside `Screen_FrameInput`, **57 are the literal `0`**; only 15 restore
a remembered screen (11 `g_menuPrevScreen`, 2 `g_screenIdSaved`, 2 `g_sliderPrevScreen`), and
none of those is the village. So a screen opened from the sidebar over the village exits to the
**map**, and the village goes with it — which is precisely the third thing the player reported:
*"when you close that dialogue it will close town square and that dialogue, probably something
to fix so it only closes the dialog you opened, but list that in future bug fixes that diverge
from the game."*

`Machine::apply_at` truncates the stack to the depth that acted, which reproduces that
structurally rather than as a special case: our stack behaves like one byte exactly where the
original has one byte, and stays a stack everywhere else. **Not fixed, on the owner's explicit
instruction** — catalogued as `docs/bugs.md` B63, with the note that the switch belongs on
`Options`' `Quirks` and not on `Tables`, because `Tables` is hashed into the save header and a
quirk there would invalidate every existing save.

**The three minimap modes, in the same branch and off the same table.** *"Most of the minimap
options aren't working, those should be quite easy to implement, it's just the same minimap
with different colors based on food, happiness and population (or sickness, not sure)."* They
are implemented now, and three things came out of it that the documentation had wrong.

**The county fields are `+0x01`, `+0x02`, `+0x03`, not `+0x0B1`, `+0x0B2`, `+0x0B3`.** The
`+0xB` was `0x0053F9B3` read as an *offset* when it is an *address* into `g_counties`
(`0x0053F9B0`) — an off-by-a-base that had been sitting in `docs/screens.md` §3 unchallenged.
They are three of the five bytes `Sync_CompareState` skips, which is exactly what they are:
interface state. The writer is `Minimap_ComputeBands` (`0x00451BBA`), found by scanning the
binary for divide-by-20 sites — there are two — and `Minimap_DrawOverlay` calls it first on
every draw. Labour is `+0x03` and takes 0, 5 or 6; food is `+0x02` and takes 0 or 6; happiness
is `+0x01` and is `happiness / 20`. **Happiness, not population and not sickness** — the
player's own guess was the half he flagged as a guess.

**Two of the three modes routinely index off the end of their own ramp, and that is the
shipped behaviour.** The ramp has six entries and both the food and labour bands can be 6, so
those pixels colour nothing: the food overlay is a single red mark on counties that went short
and nothing else. `Minimap_ComputeBands` *has* a branch that spreads the ration over bands
1 … 5 — behind `DAT_00553E60`, which is zeroed at `0x00497500` and toggled only in the window
procedure. A debug flag. The shipped path is reproduced and the other one is not.

**And two tables that look like one are two.** `0x004D28F8` is six bytes and `0x004D2900` is
six rows of *eight*, with two unindexed bytes between them; the realm ramp's unused second half
of each row is the used half with `0x20` substituted — the selected-county colour, prepared in
the table and then computed by hand instead. The rating ramp's direction is confirmed
**from the artwork**, independently of the code: `Misc_cty.pl8` frame 91, the strip drawn while
an overlay is up, is a six-swatch colour bar whose pixels read the table reversed, tick at one
end and cross at the other. That is the C52 move again — a measurement in a shipped file
settling a question the disassembly could only imply.

**The four mode buttons are not radio buttons**, which `symbols.md` said they were. In mode 0
buttons 1–3 pick a mode and 4 toggles the zoom; in any other mode 4 turns the overlay off and
1–3 do nothing at all, so food to happiness is three clicks. The artwork agrees from the other
side: frame `0x5C` has four buttons and frame `0x5B` has the bar and one.

**The one that keeps recurring**: the player has now been right eleven times out of eleven on
the interface, and every one of those eleven was settled by a *measurement that was already in
the file* — a slot number, a frame index, a colour argument, and this time six function names
in a row in an arm nobody had read. C46's pattern, C52's pattern, and now C59's.

**C60 — A turn had no duration. Three of the player's four reports were that one fact, and
the fourth met C58 coming the other way.**

A player, in one sentence: *"can't seem to move my army, and the merchant seems to just
teleport on end turn and the screen doesn't go dark"*. Then, correcting himself with the
sequence from the original: *"the merchants move and then it fades out then in which hides
the season change visuals just abruptly changing."* And separately: *"mouse scroll needs to
be like…half that speed, not sure if it's a game default or some cycle thing."*

**The teleport, the missing fade and half of the immobile army are one defect**, and it is
structural. `end_turn` ran the phase machine to completion inside `Machine::handle`, so every
phase of a turn happened **between two frames**. A merchant walked its entire route in the
time it took a function to return, which on screen is a teleport; there was no interval
during which a screen could be dark; and an ordered army was never seen to take a step.

C35 established that `Units_Tick` has one call site and it is the frame loop, immediately
after `Turn_Tick`, never reading `g_turnPhase`. **The half that was not spent is that the
dispatch therefore happens *many times*.** The seven phases originate the game's own orders
and then wait for the units they started to stop moving; those waits are not bookkeeping,
they are the **pacing**, and they are the reason a march is something a player can watch.
C35's own note said the correction was about the dispatch and not necessarily the pacing.
It was about both.

`TurnStep::Running` is the fix: one `Turn_Tick` / `Units_Tick` pair per fixed tick, the map
drawn between every pair. On the England position a turn is **48 frames**, and the six
shipped merchants are each seen on several distinct tiles across them — asserted, along with
the property that matters more, which is that the turn spread over frames produces *exactly*
the state the all-at-once turn produced. Spreading a turn over frames is a display change and
must never be a simulation one.

**`Units_Tick` also runs on frames that are not part of a turn**, and that is the other half
of the immobile army. The original's loop calls it whenever the game is up, so an army the
player orders walks away while he watches. Ours reached the sweep only from inside a turn, so
an order was accepted, `moving` was set, a path was written *and drawn*, and nothing moved
until End Turn. It is bounded by `moveAllowance - movesUsed`, so a player who sits on the map
gets no extra movement — asserted over a thousand idle frames.

**The fade is real and it is entirely in the palette.** `FUN_004B0CB4(restoreScreen, rawFlag,
palPtr)` has exactly two call sites in the binary, both on the turn boundary: `Turn_Tick`
phase 7 with `rawFlag = 1` after `Season_Advance`, and `FUN_0049A3E6` with `0` after
reloading the seasonal art. The two branches differ by a factor of four — a fade to **one
quarter brightness and back** — and the stepper at `0x004B0E03` moves each channel by at most
12 per step over palette entries **10 … 245 only**, which is why the chrome stays lit while
the map dims. Sixteen steps each way. No dither table, no 50 % blit: `l2_view::fade` produces
a `Palette` and `Canvas::to_rgba` does the rest, so a fading screen draws exactly what it
always draws and answers `Screen::fade` instead.

**What the fade is *for* was inferred and is now confirmed.** It was recorded as cover for
the seasonal art reload and the autosave, from the call sites alone. The player, who has
never seen that reasoning, says it *"hides the season change visuals just abruptly
changing"*. Two unrelated sources meeting is what promotes an inference, so our base render
is now held back until the fade bottoms out — the art changes in the dark, which is the whole
job the effect is doing.

**The army he could not move was a second defect, and two agents found it from opposite
ends of the same afternoon.** This one started from *"can't seem to move my army"* and the
other from *"if I click a merchant… it will open up the tax window"*; both arrived at
`g_pickedTileUnit` being read out of the tile record, and both replaced a nine-pixel marker
box with a tile pick. **C58 is the entry for it** and its version is the one that stands,
because it adds the drawn sprite's opacity mask underneath the tile — a figure is anchored on
the tile's bottom vertex and stands up over the tiles behind it, so the mask can add an answer
where the tile has none. Nothing here re-argues that.

**What this half contributes is the measurement and why nothing caught it.** The near-zoom
army frame is **40 × 32** and the box was **9 × 9**; after the fix, **460 of the figure's 504
opaque pixels resolve to the tile it stands on**, asserted against the user's own
`Sprite1a.pl8`. And the reason no test saw it: every test runs on `Assets::placeholder`, where
there is no sprite sheet, `draw_unit` returns false, and the fallback marker is drawn *at the
tile centre* — **the one configuration in which the old hit test and the picture agree**. The
suite was not weak; it was run in the only world where the bug does not exist. So the
assertion had to be install-gated, and it compares the pixels the sprite paints against the
tile the pick resolves them to, rather than checking that a click at a chosen coordinate
works. **Two agents converging on one defect is cheap; a test that only passes because the art
is missing is the thing to keep noticing.**

**A third defect fell out of looking**: `pick_tile`'s diamond used `tile_w / 2` and
`tile_h / 2`. `Map_PickTile` divides by `g_mapTileHalfStep` and `g_mapRowStep` — the half
**pitch** and the row step. The near tile is 58 wide and the pitch is 60, so the diamonds were
two pixels narrow and **did not tile the plane**: 56 dead pixels around every tile centre.
A `None` from `pick_tile` is not a refusal — the click falls through to county selection — so
a march order aimed at one of them quietly reselected a county. The seams are sparse and are
*not* on the line between two tile centres, so the first assertion written for them passed
with the bug still in; the one that stands sweeps a tile's whole neighbourhood.

**The scroll was three times too fast, and the player was right that it is both a default and
a cycle.** `Map_EdgeScroll` is called unconditionally every frame from five `g_screenId` arms
of `Screen_FrameInput`, so the *detection* runs at frame rate; the *movement* is gated inside
`Map_ScrollStep` by `Map_ScrollThrottle` (`0x004BBBE3`), which is a wall-clock minimum
interval on `timeGetTime`:

```c
q = (100 - g_optScrollSpeed) / 10;
if (q >= 10) return 0;                 /* speed 0 never scrolls */
if (g_screenId == 0x10) q += 2;
if (q * 12 + 2 > elapsed) return 0;
```

`g_optScrollSpeed` (`0x0053F234`) is a 0 … 100 slider in steps of ten shown as 0 … 10, and
its **shipped default is 60** — written by the options-defaults routine at `0x004AE310`,
which is unnamed in `symbols.json` and also writes `g_optGameSpeed = 90` and the settings
magic `0x7EC`. Sixty is 50 ms, which is **20 tiles a second**. Ours scrolled one tile per
fixed tick: 62.5. He said half; it was a third. **[V]** — decoded from the binary, not
inferred.

**The move-order click guard is a bug we do not have, and that is worth writing down as
loudly as a bug we do.** `g_hoverDamper` was renamed `g_moveOrderClickGuard`: forty frames of
deadness so the press that *opens* move-order mode is not read again as the press that
*confirms* the destination. It exists because `Screen_FrameInput` polls the button's **level**
every frame. Our `Event::Click` is edge-triggered — one event per physical press — and
`Map_Click`'s army branch returns, so the selecting click cannot reach `Map_ConfirmMoveOrder`
in the same call. Porting forty frames would have been porting the shape of a defect in an
input model we do not use. What landed instead is an assertion of the property the guard
protects.

**And the path markers are the game's own art now**, which matters beyond appearance. A
player: *"there are colored dot images for the army walking dots."* `Map_DrawPathMarker`
(`0x004081A6`, C49) draws `Flags1a.pl8` frame `0x38 + n` where `n` is the accumulated cost.
`docs/armies.md` §2.3 marked the mechanism **[V]** and one clause **[I]**: *"that frame
`0x38` is specifically the grey one — nobody has looked at the sheet."* Somebody has now.
Frames `0x38 … 0x4E` are **23 frames of one 15 × 15 silhouette**, 177 opaque pixels each —
one picture recoloured, which is what "indexed by cost" predicts and which a set of different
pictures would have refuted — and **`0x38` is the only frame in the run with no colour in it
at all**. The count of coloured pixels then climbs from 13 to 39 across the ramp. `0x4E`, the
castle marker, is the opposite extreme: not one grey pixel, a different picture. The
inference is now a measurement. So **the cost selects the colour** — not the realm, not the
shield, not the unit's kind.

That last one is the reason to draw them at all. `Unit_OrderMove` writes nothing when no path
is found, and an unreachable destination is an *accepted* order with an empty path — so a
refused order, a hopeless one and a good one all looked identical. **If the player cannot see
whether his order was taken, he cannot tell our bug from his own mis-click**, which is
exactly the position this report started from.

**Four reports, four defects, and the tests were green throughout.** Two of them were
invisible because the test suite runs without the game's art, one because nothing in it
counts frames, and one because nothing measured a rate. The pattern under all four is the one
C46 named: *a measurement and a word beside it that nobody checked agreed* — 9 × 9 beside
40 × 32, `tile_w` beside `pitch`, "one tick" beside 50 ms, and a turn described as a loop
when the thing it models is a frame.

**C61 — We reproduce artwork and skip behaviour. A player found two missing input arms on one
screen in one evening, and counting the rest says we reproduce 43% of what the original does
with a mouse.**

Three things happened in an hour and they are one thing. Taken separately each is a small bug;
taken together they are the measurement this project had not made.

**The reports.** *"The original has the army steps on hover, ours just populate on click."* And a
minute later: *"you cannot deselect an army."*

Both are `Screen_FrameInput`'s screen-`0x10` arm, which is four clauses long and which nobody had
read:

```c
if (g_screenId == 0x10) {                       /* the map, in move-order mode */
    if (sync_latch || turn_not_yours) { g_screenId = 0; redraw; }
    if (Map_EdgeScroll()) return;
    if (g_mouseLeftPressed && g_moveOrderClickGuard < 1) {
        g_screenId = 0; DAT_0056D64C = 1; Map_ConfirmMoveOrder(); }
    if (g_mouseRightReleased) { g_screenId = 0; g_redrawRequest = 2; }
}
```

The last line is the deselect. The hover is the other half: `Screen_DrawWidgets`' `0x10` arm is
`Map_HoverUnitTarget()` and nothing else — **where every other screen draws a widget table, this
one recomputes the route under the cursor.** It decrements the click guard, marks the route
through `Path_MarkPreviewTiles` (`0x004A91BA`), and re-runs the descent only when the hovered tile
changed. `Map_DrawPathMarker` (`0x004081A6`) then draws a ball on every marked tile **and clears
the mark as it draws it**, which is why the trail does not accumulate as the cursor sweeps.

`Path_MarkPreviewTiles` is the **only writer of tile bank bit `0x40` in the whole binary**, and it
runs only from the hover, which runs only on `0x10`. So the gold balls exist in move-order mode,
they show the route you have *not yet committed to*, and they vanish the moment you commit.

**We drew the same artwork from the opposite end.** The agent that did the path markers
established the sheet thoroughly and correctly — `0x38 … 0x4E` are 23 recolourings of one 15 × 15
ball, `0x38` is the one with no colour in it, the cost selects the colour — and then wired it to
`unit.path`, the *ordered* path. A picture the original never shows, drawn from the right sprites
in the wrong direction at the wrong time. **The sprite sheet was read and the behaviour was not**,
and that sentence is the whole entry.

**The correction that was itself wrong.** This entry was drafted once already, and the draft was
wrong.

The instruction was to remove our county-selection convenience — a click on grass selected the
county, a second click opened its tax panel — because a player had reported it: *"there's some
weird thing where if you click anywhere on grass it opens up the tax window too."* Reading
`Map_Click` produced a "last arm" that selects and recentres and opens nothing, and a conclusion
that C58 had been wrong to say there was no such arm. Code was written, tests were rewritten to
assert it, and C58 was edited in place to apologise for a claim that was correct.

**There is no last arm.** `Map_Click` ends in its terrain ladder; the tail is
`else { DAT_0056D64C = 0; }`, a scroll latch. The code quoted as a free-standing arm is the
**prologue of the industry branch** —

```c
else if (g_counties[g_pickedTileCounty].owner == g_localPlayer) {   /* flags & 0x80 */
    if (g_pickedTileCounty != g_selectedCounty) {
        if (g_counties[g_pickedTileCounty].townTile == 0) return;
        g_selectedCounty = g_pickedTileCounty;
        Map_CentreOnTile(g_counties[g_pickedTileCounty].townTile);
    }
```

— guarded by tile flag `0x80` **and** by the county being yours. `Map_Click` writes
`g_selectedCounty` three times, in the village, industry and merchant branches, exactly as C58
said. C58 needed no correction and has been left alone.

**Selection from the map is a side effect of arriving somewhere. It is never a verb of its own.** A
click on plain ground, on sea, on a foreign county, or on your own county away from its town, its
fields and its buildings changes nothing at all.

What is worth carrying is not the fact but the shape: **the correction written to fix a misreading
was itself a misreading of the same function, in the same session, by someone who had been told to
be careful.** It was caught by re-reading the decompilation before committing, which is the only
thing that has ever caught one of these. Three greps that find nothing become a claim; one grep
that finds something overturns it; and a fourth reading overturns that. `docs/method.md` §4.

**What was ours, and is now gone.**

- **The county-selection arm and its tax panel.** A click on plain ground now does nothing, and
  `a_click_on_plain_ground_changes_nothing_at_all` asserts it over the whole kingdom.
- **"Click the selected army again to cancel."** Removed, and this one is instructive. It was a
  reasonable-looking convenience *and it was standing where the real deselect goes*. In the
  original that click is a destination, not a re-selection: move-order mode is `0x10`, so
  `Map_Click` is unreachable, and `Map_HoverUnitTarget` has already cleared `g_moveOrderAvailable`
  for the tile the army stands on. Same outcome, different mechanism — and the mechanism
  generalises to every tile the fill never reached, which the convenience did not.
- **Ordering an unreachable destination from the map.** `Unit_OrderMove` really does accept an
  order with an empty path — `docs/armies.md` §2.3 is right — but a human click cannot reach it,
  because the hover gate stands in front. The test that asserted otherwise now asserts the
  acceptance in `l2-kingdom`, where it happens, and the gate on the screen.

**What was missing, and is now there.**

- `Map_HoverUnitTarget` (`0x004A8E0B`) — the route under the cursor, recomputed on pointer motion,
  descending a flood fill run **once** when the army was picked (`Map_BeginMoveSelection`,
  `0x0043723A`), exactly as the original does it.
- The right button cancels a selection (`0x10`'s fourth clause) instead of opening the information
  panel, which is screen `0`'s arm and was firing in both modes.
- `Map_Click`'s village and industry branches select the county they belong to, which they did in
  the original and did not here.

**And note what is deliberately still absent.** A click on empty ground does **not** deselect. It
is the obvious fix, it is what a modern game does, and it is not what this one does — that click
is `Map_ConfirmMoveOrder` and it either places an order or returns. Putting it in would have been
the same invention as the tax-panel convenience, made in the opposite direction and for a
better-sounding reason.

`g_moveOrderClickGuard` (`0x00553ECC`, 40 frames) is also absent, and that one is a difference of
model rather than an omission: it exists because `Screen_FrameInput` polls the button's *level*
every frame, so one physical press reads as a click on every frame it is held. Our `Event::Click`
is edge-triggered. Recorded here rather than dropped silently.

**The number.** The user's inference was that if hover-versus-click was missed, the connection to
the original is weaker than our documents imply. It is. Three screen groups were enumerated arm by
arm out of the decompilation — every hotspot, hover, drag, double-click, right-click and key — and
compared against our source:

| screen group | arms the original has | we reproduce | |
|---|--:|--:|--:|
| village, the two drag screens, the job popup | 22 | 16 | 73% |
| the right column, the menu bar, the county panels, `0x04`, `0x11`–`0x13` | 114 | 64 | 56% |
| the battlefield (`0x28`, `0x29`, `0x2A`, `0x2B`) | 49 | 0 | 0% |
| **total** | **185** | **80** | **43%** |

The battlefield zero is honest rather than alarming — those screens are unbuilt, and
`screens/battle.rs` is the campaign-map *prompt*, not the battle. Excluding it, **80 of 136, 59%.**
That is the number to argue with, and it is the first time one has existed.

**The denominator is provisional and is being corrected downward.** An exhaustive scan of every
`mov byte ptr [g_screenId], imm8` — 212 sites, `0x00`…`0x45` — finds **no writer of `0x28` at
all**, and no decompiled function assigns it either. Its `Screen_FrameInput` and `Screen_Draw`
arms are dead code, so the battlefield is three live screens and the arms audited on `0x28` sit
in a denominator they do not belong in. Counting arms cannot detect that; counting *writers of
the screen id* can, which is a hole in the audit's own method and is recorded in
`docs/agents.md`. **Do not quote 43% until it is regenerated** — the corrected total goes into
`tools/figures/figures.js` so it cannot go stale the way 34 other figures did.

One of the three inventions below is also worse than described here. Screen `0x12`'s arm is
**entirely multiplayer** — a sync latch and a timeout that returns 0 unconditionally when
`g_multiplayer == 0` — so in single player it does nothing at all, and the prompt's only exits
are the tick and the cross in its widget table. Our right-click-to-Decline is not an untested
addition to an existing arm; it stands where the original has **no input path whatsoever**. It
is being removed, and recorded as *an invention removed* rather than as a bug fixed — which is
the first countable data point for the question the arms file exists to answer.

Three patterns fall out of it, and none of them is "we were sloppy":

1. **The gap is at the level of *behaviours within* a screen, not screens.** Every screen module is
   supposed to open with the painter's address; no behaviour has any such convention. Measured: 10
   of our 16 screen modules cite at least one address in their module docs, and `map.rs` — 74 lines
   of header, the screen both of today's misses live on — cited **none**.
2. **Right-click is the systematically missed gesture.** Village group: 2 of 4 missing, and one of
   the misses is *wrong* rather than absent — a right-click while carrying peasants leaves the
   village instead of cancelling the carry. Right column: 3 of 12 missing. Battle: 6 of 6. "Right
   click exits" was learned early and applied everywhere; the original uses that button for four
   different verbs.
3. **We implement the arms a feature needs and stop.** The armies work end to end and hover was
   missing. The ration slider steps, drags and double-clicks in the original; ours steps. The
   division screen has 18 of its 20 arms and moves ten men per click where the original moves one.

Three inventions were found too, which is the same failure pointing the other way: right-click to
Decline on screen `0x12` (that arm has no right-button test at all), our sidebar hover highlight,
and the two conveniences removed above.

**The rule that came out of it.** The user, and it is now rule 5 in `CLAUDE.md`: **if we implement
a feature we must find its equivalent in the old binary's functions.** It is rule 4's other half —
rule 4 governs what we may *claim*, this governs what we may *build* — and "we could not find it"
is a finding to report, not a licence to invent.

Stated rules do not hold on this project; checked ones do. The numbering protocol was written
after four collisions and did not prevent the fifth. `git add -A` is blocked by a hook because
guidance was not enough. Five of six tool failures returned clean, plausible, wrong answers and
every one was caught by something external contradicting it. A sixth rule in a list has the
enforcement of the ones that failed, so the proposal for what would check it is in
`docs/agents.md`, and the tables above are its first data.


**C62 — A person can pick Ireland and play Ireland. Building the second world constructor
found five things about the first, and the fifth is the one worth keeping.**

`Map_InitScenario` (`0x004676E0`) is written: `crates/l2-scenario/src/newgame.rs`. Pressing
*Start* on the custom page now runs `Game_NewGame`'s own three steps in its own order — the
world, then the twelve options, then one immediate `Season_Advance` — so the slot the map
list highlights is the world the campaign screen opens on, from an empty `Game` and with no
save anywhere in the path. All 44 shipped maps start and take a turn
(`crates/l2-game/tests/newgame.rs`). The line the setup page drew about itself,
*"NOT IMPLEMENTED: STARTING ON A MAP OTHER THAN THE SAVE'S"*, is gone.

**The check is the two constructors held against each other.** England out of
`L2_maps.dat` and England out of `lastturn.sav` are two readings of one map from two files
authored separately, and they agree exactly on everything the map decides: **fourteen town
anchors, fourteen adjacency lists, 280 field tiles, 56 industry resource bytes, six merchant
routes, six merchant start counties and six merchants**. The county plane matches on all
4,096 tiles; the flags plane differs on exactly the 56 dwelling-plot bits, which is one
season of `County_UpdateDwellings`; and the terrain plane differs on **67 tiles in six
classes, every one of which names the pass that made it** — 42 field tiles the season grew
or the lord repainted, 5 wood sites stepped from idle to working (exactly the five owned
counties), and 20 castle tiles stamped from the bare plot `0x14` to a standing keep `0x17`
by `FUN_0046826C`, which is keyed on the castle's *level* and is deliberately not the world
builder's.

**Four corrections to what was written down.**

* **`FUN_00497E65` is `PlayerStart_Shuffle`, and it is why which realm you play changes
  every game.** `docs/environment.md` recorded the realm→county assignment as *rolled per
  game* — an observation, from two independently created England saves disagreeing — and
  deliberately excluded it from the fixture's fingerprint. This is the code: between
  `Mercenary_Init` and `PlayerStart_Compact`, it re-deals the live start-table entries into
  each other's slots with a random offset and a forward probe. The start *counties* never
  move; only who gets which. Reproduced on our own `Pcg32`, because the original's two
  31-bit LFSRs are not in any save — the shape is the original's and the stream cannot be.
* **`FUN_0046DA4B` is `County_CollectFieldTiles`, and twenty fields per county is a fact
  about the map rather than a cap on a counter.** It fills `g_countyFieldTiles` from the
  map and, when a county has a twenty-first farm tile, **razes it**: terrain 0, frame 6,
  flags zeroed, bank back to base. It stops being farmland before the first season runs. No
  shipped map reaches the branch, which is why nothing had ever noticed it; the branch is
  exercised on a synthetic slot.
* **`County_PlaceBlacksmith` does not give every county a blacksmith.** `symbols.json` said
  the weapons site *"is derived rather than authored, which is why every county has one"*.
  433 of 434 do. County 4 of slot 8 (Africa) has **no tile whose flags byte is zero at
  all** — its 57 tiles are every one of them road, boundary, rough, plot, farmland, town or
  site — and the candidate test is `flags == 0` exactly, so the original's own guard
  refuses. That county can never make a weapon. C21's shape again: a plausible generalisation
  written beside a correct mechanism, and nobody counted.
* **A new game opens at zero tax in every county.** `County_Reset` (`0x00451150`) sets the
  population, the ration, the split, the dryness, the shares and the stores, and writes **no
  tax rate**; the county record was zeroed wholesale by `FUN_0046EA28` in `Game_NewGame`'s
  preamble and nothing else puts one there. The England turn-one fixture's own bytes agree —
  `taxRate` is 0 in all fourteen counties. `symbols.json`'s `[inferred]` comment on
  `County_Reset` said *"tax 50"*, which is `dryness = 0x32` read one line off.

**And the fifth, which is a defect of ours and the reason this entry is worth reading.**
`County::farm_style` (`+0x1FE`) — the field `AI_ManageFields(0)` dispatches the *unowned*
counties on — **was never imported at all.** Every loaded game's fourteen counties arrived
at style 0 and every neutral county was farmed as a style-0 lord would farm it, whatever the
file said. It is imported now, and the map path seeds it the way `County_Reset` does,
`countyId & 1`; the two agree on 12 of England's 14, the two that differ being counties whose
lord had overwritten the seed by turn one. That agreement is also what says the offset is
right.

**The pattern, and it is the fifth instance.** After the four county fields missing from the
save encoder *and* the digest (C30), the four industry records `l2-scenario` skipped, and the
garrison relation stored on the unit and read off the county (C59), this one has a new face:
**a field can reach `CountyState` and stop there.** Deleting the single line of
`Scenario::skeleton` that carries `farm_style` into the county left the entire
two-constructor diff green — because a diff of two `CountyState`s cannot see the step *after*
`CountyState`. Two constructors agreeing is `CLAUDE.md`'s warning one level out: it proves
the seam is symmetric, not that the seam reaches the simulation. So the diff reports three
verdicts and not two — **agree**, **differ, because…**, and **both silent**, the last being a
finding — and it re-checks every field it calls *agreed* on the `Kingdom`, where the rules
read it. Both ablations now go red; six were run and all six do.
**C63 — The season, the fields and the village's clock; and a resource table that says a
filename is not evidence.**

Three pieces of the map were read and not drawn, and they went in together. Two of them
carried an explicit unmeasured assumption, and one of those assumptions was right and the
other quietly wrong in a way that would have been hard to see.

**The measurement that decided the whole first piece.** `docs/screens.md` §2.1 said the
seasonal swap is safe *"provided the four seasonal files of a bank really do share a frame
table. That is the one thing here nobody has measured."* It is a cheap measurement and it
decides whether the season is a lookup table or a real piece of work, because
`campaign::Overrides` stores a **frame index** — if frame 47 of `Town1c.pl8` were a different
cell than frame 47 of `Town1a.pl8`, every county town would revert to a quarry each autumn.
They share one: over 1,398 frame comparisons across five banks, the frame count, the canvas
anchor `(X, Y)`, the size and the shape agree in every season, and the **only** difference
anywhere is the overhang-row byte on nine `Roads1?.pl8` frames — 109, 111, 113 … 119 — by one
or two rows. Those nine are inside the crop blocks based at 108/112/116/120, so they are a
crop that grows needing a taller picture. Artwork varying, not an index moving. It was a
lookup table.

**The assumption in the same sentence that was wrong.** The same paragraph, and
`maps-layers.md` §1.1, say the `a`/`b`/`c`/`d` suffix *"**is** the season, four sets per zoom,
entries 0–31 and 32–63"*. It is the season for entries 0–31 and it is **not** for 32–63.
`g_resourceTable`'s zoom-2 half names `base2a`/`mtns2a`/`roads2a`/`town2a`/`castle2a` in all
four of its season blocks: the far view does not change with the year, and the twelve zoom-2
seasonal files on disk are never opened, exactly as `Flags1b/c/d.pl8` are not. The obvious
implementation — take the near zoom's names and swap the letter, take the far zoom's and do
the same — would have been wrong for three seasons in four, and **not visibly wrong**:
`Town2a.pl8` has 61 frames and `Town2b/c/d.pl8` have 94, so the far map would have drawn a
different sheet at every index without erroring anywhere.

The rule that comes out of it is worth more than the feature: **the resource table is the
authority on which file a bank loads, and the filename is not.** `Zoom::banks` is now that
table — a 4 × 5 array per zoom, transcribed from `0x004DA050` — rather than a suffix rule, and
`MapAssets` interns it by name, so the far zoom's repetition costs nothing and the
season-invariance is a fact in the data instead of a claim in a comment. Twenty-five names,
twenty distinct files.

**The artwork also settled which letter is which season**, independently of the loader
arithmetic: over the sixteen grass frames of each `Base1?.pl8`, `a` is 99.1 % green, `c` is
**0.3 %** green, and `d` is far the brightest. Autumn has no green in it and winter has snow.
Spring, summer, autumn, winter — which is what `(g_season - 1) * 8` already said, now with a
second source.

**The fields: a function that is its own documentation.** `FUN_0046D7F4` — named `Terrain_Set`
here, because four documents referred to it only by address — is the single writer of a tile's
`content` byte and picks the graphic in the same statement, so it *is* the terrain → frame map
and nothing had to be inferred from what the tiles look like. Two corrections to
`maps-layers.md` §5.5 came out of re-reading it: the `104` arm is a bare `else` and so catches
`0x1D` and up as well as `0x13 … 0x16`, and the third parameter is **dead** — all sixteen call
sites pass zero, including both callers of the one function that forwards it. The frame is
exactly `base + (storedFrame & 3)`.

The `& 3` deserves its own line because it is what makes the feature possible at all. Every
base is a multiple of four except 130 and 134, and `oldBase` exists for precisely those two —
so the low two bits of a farm tile's frame **never change for the life of the game**. That
turns the picture into a pure function of `(terrain, the byte the map file stored)`, and a
renderer can recompute it without tracking a tile's history. A stateful function with a
stateless answer, and the stateless answer is only visible once you notice which constants are
multiples of four.

**The village's clock, and a file that was not spare.** `Village_Animate` draws **six**
overlays and only three of them are the resource buildings' — the other three run in every
county, which nobody had noticed. The sixth is the single read of `villani1.pl8` anywhere in
the executable, a file this project had recorded as *"loaded by nothing"*: it is the iron
mine's loop, 21 frames of which the game plays 18.

`villani2.pl8`'s own frame table then confirmed the whole reading from a direction the
decompiler cannot reach. Its 44 frames fall into five blocks of equal-sized cells laid out in
rows on the artist's sheet — 26 × 29, 39 × 40, 15 × 12, 32 × 42, 19 × 18 — and the blocks are
**exactly** the five runs the counter bounds predict, start index and length, five times over,
with the three static buildings and one stub left over and nothing short. That is the strongest
evidence in the piece and it cost one frame-table dump. Counter bounds are a claim; block
boundaries in a file somebody else drew are a witness.

**The clock itself is `Tick_Pulses` (`0x004BBC80`)**, and it is not a frame counter: a 20 ms
gate on `timeGetTime` feeding a divider chain that sets eight one-frame booleans at 80, 160,
320, 640, 1040, 1280, 1920 and 2560 ms. The village takes two of them, and so does the
campaign flag. So *"once a frame and wrapped"*, which is what four documents said, was
describing the call site rather than the rate: the slow overlays run at 6.25 Hz.

**Where the clock lives, and the constraint that put it there.** `AnimationClock` is on
`VillageScreen` and is counted in fixed ticks handed to it, never in wall time.
`docs/netcode.md` D-12 forbids the simulation learning anything from a clock, and nothing in a
save or in the lockstep digest may depend on which frame of the smoke is showing — two clients
whose villages are on different frames are looking at the same county, not desynchronised.
The test asserts it directly: a hundred ticks of the clock leave `Kingdom` byte-identical.

**And a quirk switch that does not belong on `Options`.** A player asked for the county-name
emboss (B64) both reproduced *and* switchable. `docs/bugs.md` §6.3 recommends `Options` for a
quirk set and is right about every quirk it argues about — all of them change a rule, which is
where its three constraints come from. A text shadow changes no rule, so putting it in the
hashed options would be the *wrong* answer rather than the expensive one: D-12 says display
state must not reach the simulation. `Quirks` is on `Assets`, whose definition is already
*"everything the screens draw with, not part of the world"*, and §6.3a now says there are two
sets with a one-line test between them — **if flipping it can change a number in a saved game
it is behavioural; if it can only change which pixels are painted from the same numbers it is
presentation.**

**The tests, and the lesson from the two that were already on record.** This subsystem has
produced two sharp lessons about tests that ran in the wrong world, and both applied here.
Every seasonal claim is asserted against the user's own installed files, install-gated; the
emboss pair is read back off the canvas **as palette indices**, by reconstructing which of
`Ui_DrawText`'s three passes owns each pixel, so a colour that merely looked right could not
pass; and the season test **ends a real turn** and looks at the map rather than assigning to
`kingdom.season` and reading the lookup back — `docs/agents.md`'s rule that a field is only
tested if something a test reads was written by something the game runs.


**C64 — `castleBuilding` meant the opposite of its name, and four correct readers were
holding the wrong writer up.**

`County::castle_building` (`+0x1C1`) was documented here, in `docs/kingdom.md` and in three
Rust doc comments as *"the type under construction"*. `Castle_Order` (`0x00436D02`) has the
field's **only** write in the whole binary and it is one line:

```c
if (county.castleType != 0) county.castleBuilding = county.castleType;
county.castleType = newType;          /* immediately, not on completion */
```

It is the castle you **had**. `castleType` is the castle you are getting, from the moment
you order it. Nothing ever clears `castleBuilding`, which is safe because every reader is
gated on `castleDegraded`.

**What makes this C46's shape rather than a typo is that all four readers were already
right.** `Tax_CollectAll` charging *"the lower of standing and building"*,
`Siege_LaunchAssault` fighting `castleBuilding - 1`, `Army_BeginSiege` refusing on
`castleDegraded == 1 && castleBuilding == 0`, and the free-archer top-up firing on
`castleBuilding < castleType` — every one of those had been read out of the binary and
transcribed faithfully, and every one of them reads as arbitrary under the wrong name and as
obvious under the right one. Two of them were also *wrong in effect*: an assault fought the
scaffolding instead of the standing castle, and the *"this castle is under construction"*
refusal was unreachable, because our writer put a non-zero target in the field on every
order.

**Nobody noticed because nothing could order a castle.** `castle_degraded` had no writer a
player could reach — the chooser screen did not exist — so the whole block was dead code
with a green suite over it, which is C27 for the seventh time. Three further inventions came
out with it, each of which had a *"this is a choice, not a finding"* comment attached
admitting as much:

* the materials were debited **up front**; the original carts them in season by season and
  gates all castle labour on the delivery (`docs/kingdom.md` §7.5.1);
* `order_castle` **refused** when the realm could not pay; the original has no affordability
  guard at all, only *"you already have that one"* and *"that one is smaller"*;
* the work counted **up** against the workforce; the original counts `+0x1CC` down, and
  clamps the percentage to 99 whenever any work remains so that rounding can never finish a
  castle.

The comments were honest and they were still load-bearing for four other rules. **A field
that is read by four traced rules and written by one invented one is not half-verified; it
is a verified reading of an invented model.** The cheapest thing that would have caught it
is the thing that did: making the field reachable from the screen the original reaches it
from.

**C65 — A digest cannot audit the encoder it is made of. Every "is this field covered?"
answer this project has given was measured with the wrong instrument, including mine, tonight.**

`Canonical::hash_of` is three lines: `value.encode(&mut c); c.finish().hash`. `l2_kingdom::save::checksum`
is the same encoder. **The lockstep digest is not a check on the encoder — it is a projection
through it.** A field absent from `encode` is absent from every digest, on every peer,
identically, so two timelines that have both lost it agree perfectly.

**Measured, not reasoned.** `Industry::has_resource` was removed from `save.rs`'s encoder and the
whole workspace run. **Two tests go red, and both are round trips** —
`a_game_round_trips_field_for_field` and `a_season_report_round_trips_including_its_tagged_union`.
Not one digest assertion fails. `ten_seasons_from_a_reloaded_game_are_the_same_ten` is the
strongest one we have — it saves a played game, reloads it, and compares
`checksum(original)` against `checksum(resumed)` after each of ten further seasons — and it
**passes with the field dropped**, because both sides hash the same smaller thing.

That is worse than it first reads. With `has_resource` unencoded the reloaded kingdom is
genuinely a different world — a mine with no ore behaves differently from one with ore — and ten
seasons of divergence still did not raise the digest, because the difference has to *reach some
other encoded field* before the number can move. The digest catches a field it covers going
wrong. It cannot catch a field it does not cover, and it cannot tell you which case you are in.

**This retracts a claim of mine from earlier tonight.** I wrote that removing `has_resource` or
`garrison_unit` from the encoding *"fails eight tests each"*, and offered it as evidence that the
derived census had closed C30. The number is two, not eight, and the number was never the point:
**the question was whether the digest covered the field, and the experiment could not answer
that question no matter what it returned.** A correct experiment, a wrong inference — which is
the same shape as C61's first draft, twice in one session, and the reason both are recorded
rather than quietly fixed.

**So what is the real check?** `assert_eq!(back, game)`. It works for a reason worth naming: the
`PartialEq` it uses is **derived from the struct's field list**, and the encoder is **hand
written**. Two independently maintained lists that must agree — the same shape as
`symbols_md.js`, `figures.js` and the citation lockfile, and the only shape that has ever caught
anything here.

**And it has a hole, which is C30's own hole in a new place.** The round trip compares one
fixture. A field added to the struct and not to the encoder is caught **only if `a_game()` sets
it to something a defaulted decode would not produce.** Leave it at its `Default` and both sides
are equal and the test passes. The fixture is hand maintained, so the derived half of the pair is
only as wide as somebody remembered to make the other half.

That is exactly how `County::farm_style` (C62) and `Unit::mission` (`+0x1A`, found by the AI
agent an hour later) both survived: read by the rules, written by nothing on the import path, and
equal to zero on both sides of every comparison anybody ran.

**Proposed, not built** — a source-text check, in the family that already works here. Read each
`#[derive(…PartialEq…)]` struct that crosses the save boundary, read its `encode`/`decode` pair,
and assert **every field name in the struct appears in both**. It cannot prove a field is encoded
*correctly*; it can prove none was forgotten, which is the failure that has now happened six
times. It is cheap, it goes red on the commit that adds the field, and it needs no fixture to be
clever. The alternative — making the fixture derived — cannot be done in Rust without a macro,
and a macro that generates the thing under test would be the same mistake one level down.

**Built, and falsified before being believed.**
`crates/l2-testkit/tests/encoding.rs` reads the source text: every `impl Encode`/`impl Decode`
pair, the struct's field list, and the assertion that each field is named in both halves. **220
fields across 21 types.** Two experiments, both red, both with the field named in the message:
dropping `out.bool(self.has_resource)` from the encoder — *the exact case the digest passes* —
and adding a field to `County` that nothing encodes.

Writing it reproduced this project's commonest tool failure twice, which is worth recording
because both were clean, plausible and wrong. Resolving a codec's type by short name across the
workspace matched `l2_formats::save::Unit` for `l2_kingdom::unit::Unit`, reporting twelve fields
of the raw `.sav` record as missing from a codec that has never seen them; and it matched
`l2_kingdom::trade::Order` for a test fixture in `l2-net`, reporting six more. Both were caught
by *reading the failures* rather than by counting them — 24 findings looked like a productive
first run. Resolution is now required to be same-crate, and a name that will not resolve there is
reported as unverifiable rather than answered about the wrong type.

**Two limits, stated because an unstated limit gets trusted past.**

1. **It cannot say a field is encoded *correctly*.** `out.u8(self.a)` written twice and
   `self.b` never passes this check and is wrong. It proves nothing was *forgotten*.
2. **It does not reach the importer, which is where both live instances actually are.** This is
   the part that revises the proposal as approved. `County::farm_style` was not dropped by
   `l2-kingdom`'s encoder — it was dropped by `l2-scenario` building a `County` from a `.sav`,
   a path that never touches `encode`. **This check would not have caught it, and will not catch
   `Unit::mission` either** — a field that does not yet exist on `main`, so it could not be the
   proof it was asked to be.

**And the importer's fix is not a lint at all.** It builds its structs by assignment onto a
default — `c.farm_style = s.farm_style;`, forty-odd lines of them — so a forgotten field is
silently left at zero. **A struct literal with no `..` makes every one of those omissions a
compile error**, enforced by rustc, permanently, for free, and with no scanner to go wrong. That
is strictly better than anything in this entry and it is the work to schedule: the two fields we
know about, and every future one, in a construct the compiler already checks.

So the honest summary: the source-text check **narrows** the hole on the half of the boundary it
can see. It does not close it, and the half it cannot see is the half that has actually bitten us
twice.

Until the importer is converted, the rule to state plainly wherever coverage is claimed: **"the
digest covers it" is not a sentence anybody can support by running the digest.** Ablate the
encoder and read *which* tests go red, not how many.

**C66 — The raise-army screen was floating over the wrong picture, and the button that
raises an army was on the screen nobody had built.**

A player: *"The hire an army is pretty botched at the moment. Instead of the blacksmith with
a listing of their tools, it's just a weird popup with a lot of placeholder stuff."* Twelve
out of twelve, and both halves of the sentence were literal.

**The finding is one line of `Screen_Draw`.**

```c
else if (g_screenId == '\n')   { Screen_Armoury(firstFrame); }
else if (g_screenId == '\x17') { if (firstFrame == 1) Screen_Armoury(1); Screen_RaiseArmy(); }
```

**One painter, two screens.** `Screen_Armoury` loads `armoury.pl8`, paints the room, and ends
with `Palette_Set(armoury.256)`; `Screen_RaiseArmy` then draws a `Ui_DrawBox` on top of it and
never clears. So the levy window is a window *on the armoury*, in the armoury's palette — and
`screens/army.rs` answered `is_overlay() == true` with no palette, which put it over the
campaign map in the campaign colours. That is the *weird popup*, exactly: a box with nothing
round it. The *blacksmith with a listing of their tools* is the room it should have been
standing in, and the listing is `FUN_00418426` — six weapons at fixed positions on the walls,
**each drawn only when the realm owns one**, so the picture is an inventory.

**And the raise button was ours and in the wrong place.** `Army_RaiseConfirm` is not reachable
from screen `0x17` at all. Its callers are `FUN_00435AE8`, bound to hotspots 6, 7 and 8 of
`g_armouryHotspots` — the three words down the armoury's right-hand edge, `L2.eng` 69/6, 69/7
and 69/8: **Create, Change, Cancel**. Screen `0x17`'s entire widget table is three records: a
*Continue* button that goes to the armoury, and the tick and cross of the mercenary offer.
Filing `0x0A` as a shell therefore did not leave a screen unbuilt; it left **the door out of
the levy screen unbuilt**, and `screens/army.rs` grew four buttons of our own to stand in for
it — AUTO-EQUIP, UNEQUIP ALL, RAISE and CANCEL — every one of them now deleted and every one
of them with a real home.

**Three smaller corrections came with it, and the first is the shell table's own.**

* The armoury's `L2.eng` group is **69**, not 16. Group 16 is the twelve mercenary
  nationalities and the painter never touches it. The row also called `arm_grid.pl8` a *"buy
  grid"*; **nothing on either armoury screen is bought.** Weapons are made in a county and
  paid for in iron and wood, and the armoury is where men pick them up. The four functions
  `docs/hypotheses.json` had named `Armoury_Buy*` are `Levy_EquipOne`, `Levy_UnequipOne`,
  `Levy_UnequipAll` and `Levy_EquipAll`.
* **`tools/oracle/widgets.js` had the plus and the minus the wrong way round**, and the
  database inherited it. Its conventions note said *"68/66 minus and plus"*; the only other
  table in the binary that uses the pair is the diplomacy gift row (`0x004DD9D0`), whose
  frame-68 record carries hotspot id 1 and whose handler `FUN_00436372` adds ten crowns for
  id 1. **Frame 68 is the plus.** The tool is corrected as well as the database, because the
  tool is what would have said it again — the same shape as C3's warning about a hypothesis
  generator, one layer down.
* **The slider does not re-seed the basket.** `screens/army.rs` said `Levy_SliderClick` called
  `Levy_SetPercent` and then `FUN_004AA90A`, and that *"re-seeding the basket is what makes a
  slider move throw away the equipment"*. The tail of that function is two statements and
  neither is that call. Equipment *is* thrown away, by the **door into the armoury**, which
  re-seeds on every entry — so the visible behaviour survived the correction and the sentence
  explaining it did not. A conclusion that survives a wrong reason is the most expensive kind
  to leave standing.

**`Levy_AutoEquip` at `0x004AAD5F` named nothing.** The address is inside `Battle_AutoResolve`
(`0x004AAD07`, 1,443 bytes), the symbol is in neither database, and the only place in the tree
that cited it was the AUTO-EQUIP button's doc comment. The *rule* is real and the AI runs it;
no button in the game does, which is why the button that cited it was ours.

**What the test discipline caught, and what it could not.** `docs/agents.md`'s rule — *a field
is only tested if something a test reads was written by something the game runs* — predicted
this exactly. Deleting the importer's `realm.weapons = r.weapons` broke **nothing** in the
workspace: every weapon assertion in the tree was downstream of a fixture the test had written
itself. `every_imported_realm_holds_the_stocks_the_file_holds` closes it and goes red on that
deletion, naming the England fixture's `{0, 0, 50, 50, 50, 0}`.

The save side was already covered, and *how* is worth recording. Breaking the encoder's
weapons write turns six tests red — and **not one of them is the lockstep digest**, because
`Canonical::hash_of` is the same encoder: both timelines lose the field identically and hash
the same. The check that works is the plain `assert_eq!(back, game)`, a derived comparison that
does not route through the encoder at all. *A digest cannot audit the encoder it is made of*,
and any future "is this field covered?" question has to be asked of a struct comparison.



**C67 — Two rules written down in two documents, and nothing joined them: an army ordered
onto its own castle stood on the tile for ever.**

`docs/armies.md` §9's target table has the rule — *your county → `Army_Garrison`, anybody
else's → `Army_BeginSiege`* — and §2.2's tile table, which is what somebody implementing
movement reads, describes plane-0 `0x80` as *"the move ends; `Unit_TrampleTile` charges 7,
conditionally"* and does not mention that the tile might be a castle. So
`movement::step` trampled it and stopped, `Army_Garrison` was implemented nowhere, and
`conquest::attack_county` answered a friendly county with `Refusal::AlreadyYours`.

**Nothing in the game could produce that order**, so nothing was wrong. A person has to click
the castle and the campaign UI has no such click yet; no AI raised an army at all. The moment
AI step 7's garrison pass landed it produced the order several times a turn, and on the England
fixture the result was one army per AI realm frozen one tile from its own castle for **forty
turns**, with the castles still empty. It cost about an hour to find and four lines to fix.

Two things worth carrying:

* **The two tile bits are not what their names say.** `flags::CASTLE` (`0x40`) is *the county
  town* — C25 said so from `L2.eng` and this is the second time it has bitten — and `0x80`
  with terrain `0x15…0x19` is the castle building. Walking onto the first takes a county;
  walking onto the second garrisons or besieges. `Step::reached_castle` was named for the
  first and there was no field for the second.
* **This is C59's shape exactly, one level up.** C59 was a *relation* stored from one end and
  read from the other with nothing joining them. This is a *rule* stated in one section and
  absent from the section somebody implementing it would read. Both were invisible behind a
  green suite for the same reason: no code path in the shipped engine reached them.

The generalisation, which is worth more than the fix: **a rule that only one caller can reach
is untested until that caller exists**, and the AI is the caller for a large fraction of this
engine's rules. Fourteen turn handlers were named and four were run; the ten that were not
were holding shut every rule only they call. `docs/plan.md` §2.4 argued that from the outside;
this is the same argument with a number on it.

**C68 — Four of the AI's inputs are written by a module that does not exist, and two of its
handlers can therefore never fire.**

Asked directly — *for every field these handlers read, what writes it in a real game?* — the
answer for `crates/l2-kingdom/src/ai_army.rs` is that four fields have exactly one writer and
that writer is `l2_kingdom::diplomacy`, which is not a module. `crates/l2-kingdom/src/realm.rs`
links to seven of its functions in doc comments and every link is dangling.

| field | its only writer | what is unreachable without it |
|---|---|---|
| `Realm::pairs[].standing` | `Diplo_Init`, `Diplo_Offend`, the seven reply handlers | **AI step 10 entirely** — the raid wants a rival below −10 |
| `Realm::war_target` | `Diplo_Offend` | the same, and step 9's halved population floor |
| `Realm::ally` | `Diplo_FormAlliance` | mission 6 *assist ally*, and `Diplo_ActionAllowed`'s grudge |
| `Realm::target_county` | `Diplo_PayForHelp` | step 9's ally-request branch |

So the raiding party — step 10, the whole of `FUN_004A0015` and mission 7 — is implemented,
dispatched, unit-tested and **can never fire in a played game**. That is C27 restated for the
AI's war, and it is written down here rather than left to be discovered because the suite is
green either way.

`crates/l2-game/tests/ai_war.rs` holds both halves as one test: nothing moves a standing off
zero in forty turns, *and* the raid goes out the moment something does. The first assertion is
designed to **go red when diplomacy lands**, which is the only way a gap like this announces
that it has closed.

**C69 — The options screen is four screens, the group switch spans two homes, and the
toggle list is generated from `docs/bugs.md` rather than kept beside it.**

The brief was *"build the options screen, and give it a group switch for the original's
bugs"*. Three things came out of it that the brief could not have known, and one of them
changed the design after it was half built.

**There is no options screen.** The Options drop-down (`L2.eng` group 2) opens **three**
separate modal panels, a fourth hangs off the Help menu, and two more entries open the shared
value spinner — six controls behind one menu with three `g_screenId` values between them:
`0x39` Advanced (`Screen_AdvancedOptions` `0x00414F68`, group 50), `0x42` Sounds
(`0x0041515C`, group 51), `0x43` Display (`0x004152EA`, group 52), `0x31` Help
(`0x004154EA`, group 45), and `0x21` for both speed spinners (`Screen_SliderBox`
`0x0040CD58`). All four panels were already rows of `screens/shells.rs`, and **each row's
`unfinished` string said exactly what was missing** — *"the four Yes/No values from group 18
at x = 0x140"*, *"the two values, and the F5 note that only shows in windowed mode"*. Those
four sentences are what `screens/options.rs` answers; the shells are gone. That is the second
time the shell table has paid for itself as a to-do list written by the binary rather than by
us (C22 was the first).

**Group 50 has four rows and this document said three.** `docs/bugs.md` §6.4 named
*"three behaviour switches"* — Advanced Farming, Foraging, Exploration — as *"group 50 indices
1 … 3"*. The group holds **five** strings and `g_advancedOptWidgets` (`0x004DDC10`) holds
**four** widget records; the fourth is *"Fight humans only?"*, and it changes a rule
(`FUN_004A6A30` auto-resolves a battle the local player is not in when the byte is 0). **[V]**
both ways — the string count out of `L2.eng`, the record count out of `.data`. B55a had been
discussing that same option's *save* behaviour two sections earlier without either half
noticing the other. Nothing turned on the number; it was simply wrong.

**Where a quirk lives, and the two homes.** The rule going in was *`Options`' `Quirks`, never
`Tables`*, and the reason is exact: `save::ruleset_fingerprint(tables)` is hashed into the
save **header** and `decode` refuses a mismatch, so a quirk on `Tables` invalidates every
existing save the day it is added — and frames a quirk as a *rule*, which it is not.
`Options` is in the save **body**, and the save body *is* the per-tick lockstep digest
(`save::checksum` is `Canonical::hash_of(kingdom)`), which is exactly where something that
changes what the simulation computes belongs.

Half way through, another agent argued that a *presentation* defect fails every part of that
argument — a text shadow cannot change a turn, and `netcode.md` D-12 forbids display state
reaching the simulation at all, so the hashed options are the **wrong** home rather than the
expensive one. That is right, it is now §6.3a, and the test is one line: *if flipping it can
change a number in a saved game it is behavioural; if it can only change which pixels are
painted from the same numbers it is presentation.*

So there are two homes at very different prices — a behavioural quirk costs a `save::VERSION`
bump, a handshake field and a replay stamp; a presentation quirk costs one `bool` — and **the
asymmetry is the hazard the split creates**. A rule variation filed on `Assets` because it is
cheaper there would be invisible until a multiplayer desync.
`crates/l2-testkit/tests/quirks_catalogue.rs` therefore asserts the home, both ways, and fails
outright on a quirk implemented in both. That assertion is worth more than any of the switches
it guards.

**A set bit means *fixed*, so faithful is zero.** `l2_net::Quirks` is a `u64` bitfield and the
sense is inverted deliberately. It looks backwards for about ten seconds and then pays twice:
the default is byte-stable, so adding a quirk next month changes no byte a faithful game
writes and costs no version bump; and an unknown bit read by an older build is 0, which is the
original's behaviour — the answer that cannot be wrong, because it is the answer the original
gives. `docs/bugs.md` §6.3 asked that the bump be paid once rather than once per bug; that is
how it is paid once.

**Faithful by default**, on §6.5's argument rather than on taste: the original is the oracle,
the bugs are load-bearing on a balance nobody has measured, and the default becomes the value
the whole corpus of saves and replays is recorded under.

**The part that will still be true in a month is the generated list.** A hand-kept list of
toggles beside a catalogue of ninety entries drifts within a week, and the drift is invisible
because the code compiles either way — this project has watched thirty-four documented figures
go stale at once, and watched a comment in `ai.rs` state an expired constraint and set the
AI's priority for weeks. So `quirks_catalogue.rs` reads `docs/bugs.md` §2 and both switch
lists **as text** — not by linking them, because code that merely compiles is not evidence
that two documents agree — and asserts five things: every catalogue entry has a disposition;
`Switchable(home)` means a switch in that home and no other; a `Quirk` variant that no
simulation file calls `reproduces(` on fails as **inert**; nothing on `Tables` names a quirk;
and the presentation table and its struct are the same list. It prints the corrected inventory
on failure, which is `census.rs`'s habit and the one worth copying. All five were ablated —
broken deliberately, confirmed red, restored.

**The count, honestly.** Of the **68** behavioural entries in §2 (one retracted), **14 are
switchable and wired**, all behavioural; **38 are unwired** — reproduced, switchable at a
reasonable price, nobody has done it, and the row says which file to open; **15 are
unswitchable**, each with a written reason. Three of those reasons are worth reading: the
pathfinder group (B5, B36, B37, B38) cannot be exposed per-entry because B5 makes one search's
result depend on which searches ran before it; B56, the silenced sync-digest block, is settled
by `CLAUDE.md` — reproducing a defect in the mechanism that *detects* divergence buys nothing
a player can see; and B21, B22, B23 and B55 are invisible, and belong in the model rather than
in a settings page.

**A correction found by testing rather than by reading.** B16 says *"a county that dies out
records a negative death count"*. On the season a county **loses its last person** the
arithmetic lands on exactly 0, so the recorded figure is 0 — wrong, but not negative. The
negative number appears on the **next** season, when the pass runs again over a county that is
already empty and drives it to −1. Established by walking population 0…400 × five health bands
× five happiness values × four seasons × four event modifiers: every negative case has
`population == 0` going in. Both faces are switched by the same flag and both are asserted, and
the survey is in the test rather than in a sentence here, because *"we could not make it go
negative"* and *"it cannot go negative"* are different claims — the first draft of that test
made the first claim, from a search too narrow, and was wrong. *(**Retracted in part by
C170.** The survey was exhaustive and its answer was true — of our
`update_one`, which compared the unscaled birth rate where `Population_UpdateAll` compares the
scaled one. With the original's comparison a county of one at no happiness records −2 on the
season it dies, so B16's own sentence was right. The test and the `Quirk` doc now assert that.)*

**One thing the architecture would not allow, and what it cost.** A screen is handed
`Ctx { game: &mut Game, assets: &Assets }`, and that asymmetry is load-bearing: it is what
makes `draw` unable to change anything. So the quirks page **cannot write `Assets`**, which is
where §6.3a puts the presentation half. The setting therefore lives on
`Game::presentation_quirks`, where the page can write it, and `main.rs` pushes it into
`Assets::quirks` before each frame — one authority, one projection, with a test asserting the
projection line exists so it cannot become a field the drawing code never sees. The
alternative is `Ctx { assets: &mut Assets }`, which is 97 construction sites and would let
`draw` mutate; it is not obviously wrong and it is not this branch's to take.

**And a third category, named because two of them were already being confused.** A *rule* is
what the game was started with and is in the digest; a *quirk* is one of the original's
defects, switched; a *preference* — sound, animations, scroll speed — is what **this machine**
is like and reaches neither the save nor the digest. `l2_game::game::Prefs` is that third
category, and a debug overlay toggle belongs there: not on `Options::quirks`, not on `Tables`,
because it is not a rule variation at all.

**C70 — The battlefield had no input at all, and enumerating it found a screen that cannot
be entered, a battle that starts paused, and five inventions of ours on the screen before it.**

C61 measured us at 80 of 185 input arms and put the battlefield at **0 of 49** — the single
largest hole, and the only screen group where a battle could run to a conclusion without the
player being able to do one thing about it. This is that group built, and the enumeration it
had to start from.

`docs/battle.md` §15 is the enumeration; `docs/arms.json` is its machine-readable form and
`crates/l2-game/tests/arms.rs` holds it to the code in both directions. Below is what
changed a document rather than a feature.

**Screen `0x28` cannot be entered.** The audit named four battlefield screens. There are
three. Every immediate write of `g_screenId` in the binary was enumerated — 212 `mov byte
ptr [0x004EAC50], imm8` sites, covering `0x00` … `0x45` — and `0x28` is absent while `0x29`,
`0x2A` and `0x2B` are present; no decompiled function assigns it, and the five indirect
writes can only restore a value the byte already held. It has a live `Screen_FrameInput` arm
and a live `Screen_Draw` arm, and neither can run (`docs/bugs.md` D37).

**That is the shape of evidence this project is supposed to produce and mostly does not.**
An exhaustive scan of a machine-checkable property, rather than a search that came back
empty. C58's *"three greps that find nothing become a claim"* is the failure mode; the
difference here is that the scan enumerates the whole class and *counts* it, so "not found"
and "not there" are the same statement.

**A battle starts paused, and the pause sound is dead.** `Battle_Start` writes
`DAT_0053F238 = 0xFFFFFFFF` before it raises the screen. Battle button 0 toggles that word
with a bitwise NOT, so the very first thing a player does in every battle is press pause — to
unpause it. The same function then guards a sound on `if (DAT_0053F238 == 1)`, and a word
that only ever holds `0` or `-1` is never 1 (`docs/bugs.md` D38). We had no pause at all;
our battles ran the instant they were raised.

**`H` and `V` settle a field two documents called untraced.** `docs/battle.md` §1 listed unit
`+0x09` as "unnamed and untraced". `FUN_0043C77A` writes it, from a value only a keypress
can supply, and `Formation_ComputeRect` reads it in one line: `if (field_0x9 == 1)
g_formationCols = 2`. So the two keys are **line and column**, and the byte has exactly one
writer and one reader. That is what an unknown field looks like when it is approached from
the input side rather than from the struct.

**`BattleUnit_Order`'s fifth argument is not `fromPlayer`.** `docs/symbols.json` named it
that. Its only writer is `Battle_UpdateHover`, which sets it when the hovered cell's surface
byte is 15 and clears it otherwise, and all twenty-five AI call sites pass a literal 0. Its
effect is that a missile unit of side 0 ordered onto such a cell stops short. Renamed;
*why* surface 15 is **not established** and is recorded as open rather than narrated.

**Selection is simulation state, and this is not a modelling preference.** `FUN_00478987`
walks the selection and, when it is not exactly one whole unit, **allocates a new unit and
moves the picked figures into it**. A box drawn round half a unit *splits* that unit in the
original, which changes what every later order applies to and what the AI's own sweeps see.
So `Figure::selected` is in `l2-sim` and belongs in the lockstep digest, and the whole
select-and-order path is below the screen rather than in it.

**Five arms of ours, on the screen before the battlefield, and this is the part worth
carrying.** C61 found three inventions by enumeration and had no way to count them. Screen
`0x12`'s arm in `Screen_FrameInput` is:

```c
else if (g_screenId == '\x12') {
    if (DAT_00553fc8 != 0)        { Battle_Decline(); … }   /* the sync latch  */
    if (FUN_004bbea7() != 0)      { Battle_Decline(); … }   /* an answer timer */
}
```

and `FUN_004BBEA7` opens `if (g_multiplayer == 0) return 0;`. **In a single-player game the
arm does nothing at all.** The only two exits are the two widgets of `DAT_004DDBB0`, whose
count `Battle_ChooseSettlement` writes as 2 when the local player owns the choice and 0
otherwise — so a bystander's prompt has no widgets and no exit but the multiplayer timeout.
The table holds exactly two records; `g_sliderWidgets` begins 48 bytes on, which is what
rules out a third widget hiding behind the count.

Ours had **right-click to Decline, Escape to Decline, Enter to take the field, and
answer-on-any-click for a bystander**, plus **Escape and Enter on `0x13`**. All are gone and
all are in `docs/arms.json` as `invention` with `removed: true`, which is the first time this
project can *count* the direction C61 could only name. And `0x13` really does have the
right-button exit that `0x12` does not — two neighbouring screens differing on it is exactly
what made the invention look reasonable.

**The prompt waits for ever in single player and that has been left alone.** It is what
`docs/symbols.json` records of `Battle_Decline`; a hang that is the original's is content,
and the instinct to "fix" it while removing the right-click is the same instinct that put the
right-click there.

**The check, because a stated rule on this project does not hold.** `docs/arms.json`'s
`reproduced` records and the `// arm: 0x…` markers in the Rust are compared for **set
equality in both directions**, and the two differences are reported separately because they
mean different things: a record with no marker is a claim nobody kept, and a marker with no
record is an arm nobody wrote down — which is the exact failure C61 measured. Both directions
were ablated. It is a test rather than a convention for the reason C61 gives about its own
numbering protocol: the protocol was written after four collisions and did not prevent the
fifth.

**One fixture bug caught by ablation, and it is the fifth of its kind.** The first version of
`the_right_button_on_the_battlefield_clears_the_selection` boxed the opening viewport, which
`Battle_Start` puts at cell (0x20, 0x21) — nowhere near either army's deployment marker. It
selected nobody, then asserted that nothing was selected. Deleting the deselect arm left it
green. `docs/agents.md`'s rule caught it only because the ablation was actually run:
**a check that passes is not a check that would have caught the bug**, and the assertion that
fixed it is one line saying the fixture is non-empty.

**What is deliberately not built.** The menu bar's three titles during a battle — the same
arm is missing on the campaign map, and whether `Menu_SaveGame` works mid-battle is an open
question rather than a feature. The four debug keys, three of which are gated on a flag no
shipped game sets. And `0x28`, which cannot run.

**One number in the audit was reproduced exactly and it is the one that matters.** C61
records *"Battle: 6 of 6"* right-button arms missing. There are exactly six, and the
enumeration here reaches the same six by a different route. The totals differ — 38 against 49
— and that is a counting rule rather than a disagreement about the code; `docs/battle.md`
§15.11 states the rule and says where the difference most likely is. Two enumerations agreeing
on a sub-count they were not aligned on is worth more than either total.

**C71 — "Nobody dies" was three-quarters false, and the quarter that was true was
a rule with no writer.**

The brief this began from read: *"Retreat and autocalc discard the battle. Both confirms reach
`FUN_0043BE65` = `Battle_AutoResolve` + return; neither calls `Battle_WriteBackCasualties`.
**Every casualty so far is unkilled**"* — and concluded that the campaign–battle seam is
one-directional and *"every fought war on this project is meaningless, including the
forty-turn AI war test."*

**The premise is right about the binary and wrong about what follows from it**, and one probe
measures it:

* `Battle_WriteBackCasualties` (`0x0047F474`) has **five call sites**. `FUN_0043BE65` is not
  one of them, exactly as reported — but `Battle_CheckOutcome`'s post-banner arm and
  `FUN_004782C5` are, and those are how a battle **fought to its end** returns. That path was
  already implemented (`engagement::conclude_fight` → `write_back`) and is asserted by three
  tests; ablating the write-back turns all three red, which is the check that says so.
* An **AI-versus-AI battle never enters the simulation at all.** `Battle_ChooseSettlement`
  returns 0 when neither owner is human, and the autocalc's whole purpose is to write the
  survivors into the campaign records. So the forty-turn AI war was never affected: it fights
  **five battles and kills 1,951 men** over forty turns of the England fixture, and did
  before this branch as well as after it. Every row of both worlds is byte-identical before
  and after — which is the honest answer to *"what does the map look like with casualties
  applied"*, and it is *nothing changed, because nothing there was broken.*
* Only the **early exit** discards, and there it is the original's own behaviour — with an
  asymmetry nobody had noticed: `FUN_0043BDCD`'s multiplayer arm calls
  `Battle_WriteBackCasualties` first and its single-player arm does not. `docs/bugs.md`
  B76.

**What was genuinely missing is one function and one clause, and they were missing
together.**

`g_battleWithdrawal` has exactly one writer in the binary — `UnitOrder_SiegeAttKnight`
(`0x0048D9CE`), three statements at the top of the think:
`if (g_aiMenTotal <= g_aiMenKnight && g_siegeBreachScore == 0) { withdraw }`. **That clause
was absent from `l2-sim`'s handler.** With it absent, `End::Withdrawal` could not arise in a
played game, so:

* `Battle_ReturnToCampaign`'s withdrawal branch was unreachable, and
* **`Army_WithdrawCasualties` (`0x004AD8CC`) — which was in no document and no symbol file —
  could not be noticed as missing.** It halves every troop line, wipes any line under eleven,
  spares the mercenary band, and clears the path and the move state; and it runs **above** the
  whole loser branch, so `menTotal < 50` reads the *halved* total. `crates/l2-kingdom` tested
  the threshold against the unhalved one, so an 80-man army survived at 80 where the original
  halves it to 40 and destroys it. The shipped `Readme.txt` says it in English — *"any army
  that would have less than 50 men **after** retreating is eliminated instead"* — and this
  project had quoted that sentence twice while implementing the other order.
* The absence also produced a **stall**: with the clause removed, the fought siege that now
  ends in a withdrawal at a couple of hundred ticks instead runs to `MAX_TICKS` and returns
  `Resolution::Stalled { ticks: 12000 }` — an all-knight besieger with no way in and, until
  now, no way out.

**The shape of it is C27 again and worth naming.** Three artefacts said the rule existed —
`symbols.json`'s comment on `g_battleWithdrawal`, C31 and C38, and
`l2_kingdom::victory::recount_strength`'s own doc comment listing *"the two arms of the
post-battle resolution"* among its four callers — and in a played game **not one of them
could fire**, because the single thing upstream that raises the flag had never been written.
A rule documented, tested and unreachable looks exactly like a rule that works.
`Realm_RecountStrength` is called from `turn::record` now, and the withdrawal clause and
`withdraw_casualties` are ablation-checked in both directions.

**Two things about how this was reported, added when the entry was merged.**

The brief that started it said *"every casualty so far is unkilled"*, which is a claim about
the seam. What was true was *"the solo retreat arm discards casualties"*, which is a claim
about one branch. **A true statement about one branch, promoted to a statement about the**
**subsystem** — the fourth right-output/wrong-meaning instance of the week and the first that
travelled in prose between people rather than in a tool's output, where none of the checks in
`docs/agents.md` can reach it. The defence is cheap: **name the branch**, because a scope in
the sentence cannot be widened by accident.

And the ablation that settles it needs `--no-fail-fast`. Run plainly, `cargo test --workspace`
stops after the first failing crate and reports **one** red test where there are three — so an
ablation read without it understates its own result, which is a poor way to learn how much a
line is worth.

**C72 — The engine names its own functions in `status.txt`, and it graded five of
our guesses.**

`L2.eng` is the project's strongest naming lever and it has a hard limit: it only reaches code
that draws text. The engine layer — DirectDraw, the window, the transport, the video player —
draws no strings and has been the darkest part of the binary for that reason.

It has an equivalent, and `docs/symbols.md` had already pointed at it without anyone working
it through: `Lords2.exe` writes `status.txt` beside itself. The writer is **`Log_Write`
(`0x004AFAB9`)**, **85 functions call it with a literal message address**, and the messages
are the game describing what that function is doing. `OK :DD Set resolution.`
`ERR:DP open session - user cancel` `ERR:BATTLE Data load, couldn't find `.

**Eight of those messages contain the routine's own name**, in the C convention
`ERR:<function> bad data`:

| address | the game's word | what it is |
|---|---|---|
| `0x004071A0` | `top_it` | the tall-sprite overhang blitter |
| `0x0040946D` | `gen_frame` | `Sprite_GenFrame` |
| `0x004097C5` | `gen_blank` | `Sprite_GenBlank` |
| `0x0040A127` | `gen_sprite` | `Sprite_GenSprite` |
| `0x0040A3D0` | `write_c_sprite` | the clipped twin of `Pl8_DrawFrame` |
| `0x0040A682` | `w_gen_sprite` | |
| `0x0040A80E` | `w_gen_h_sprite` | |
| `0x0040A9B0` | `w_gen_f_sprite` | |

**And five more grade names we had already committed to.** `mos_frame`, `mos_blank`,
`mos_24blank`, `write_sprite` and `place_sprite` are `Ui_DrawBoxBorder`,
`Ui_DrawBoxInterior`, `Ui_DrawTileStrip`, `Pl8_DrawFrame` and `Pl8_DrawFrameHere`. Five
independent chances for a guessed name to be wrong; **none was**. That is the first time
anything on this project has been able to mark our own naming rather than merely be
consistent with it, and it is worth more than the eight new names.

Two cautions, because this lever has the shape the project keeps getting caught by.

* **The number argument is passed plus one.** `Log_Write(message, extra, n)` prints `n - 1`,
  and every call site writes `value + 1` so that `0` can mean *no number*. A reader taking the
  immediate at face value is off by one at 85 sites.
* **A message says what the caller was doing, not what the function is.** `Screen_DrawConquest`
  and six other art loaders share `"ERR:Data load, couldn't find  "` verbatim. The lever gives
  a *subject*, exactly as `anchor.js` does with `L2.eng` groups, and the role still needs its
  own check.

`tools/oracle/logstrings.js` is the lever as a tool — `--unnamed`, `--grep`, `--fn`/`--arg` for
any other function that takes a literal string address. It reads the PE section table out of the
exe rather than hardcoding it, and takes `--decomp`/`$LORDS2_DECOMP` because the corpus is
gitignored and every agent now works in a worktree that therefore has none — `anchor.js` and
`xref.js` assume `__dirname/decomp` and are unusable from a worktree for that reason.

**C73 — `g_netCmdWriters` is 100 entries, not 112, and the check that said 112
passed for an accidental reason.**

`g_netCmdWriters` (`0x004D57F0`) carried a confident comment: *"void(\*)(void)[112] … every one
of the 112 entries is a real function start in the decompiled corpus — a mechanical check that
could have failed"*. The check ran, it passed, and it was worthless.

`Net_ApplyPacket` (`0x0043EAC1`) — the receive half of `Net_SendCommand`, unnamed until now —
dispatches through a **second** table at `0x004D5980`. `0x004D5980 − 0x004D57F0` is exactly
400 bytes, so the writer table is **100** entries and the twelve that were counted past its end
are the first twelve *handlers*. They are real function starts, so the check saw what it was
looking for.

Three further constraints, and they all agree on 100:

* the handler table's own hundredth slot is `0x004D5B10`, which is `g_syncBlocks`;
* `g_netCmdLength` (`0x004D5B90`) is `0xFF` from index 98 onward, and the valid opcodes are
  `0x01`…`0x61`;
* every entry of both tables in `0x00441050`…`0x00446D4E` is inside the network-command block.

This is `docs/agents.md`'s *"a check that passes for an accidental reason is
indistinguishable from one that passes for the right reason"*, and it is the cleanest instance
on file: **the accident was that the array being over-read was followed by another array of the
same element type.** The general defence is the one that caught it — a bound stated by
something other than the thing being bounded. Here it was a neighbouring symbol's address.

`g_netCmdHandlers` is now in `symbols.json` and both counts are corrected.

**C74 — Nobody could type anything, and the reason the arms audit missed it is
that the keyboard is not on the screens at all.**

A player wrote one line: *"I can't type my name in the start menu?"* The answer was that
**this workspace had no keyboard text entry of any kind.** `Key::Backspace` was manufactured
by `main.rs` and read by one hand-rolled `String` on the save screen; `Key::Char` reached
hotkeys and nothing else. Every field the original lets a person type into was absent, and
nobody had counted them.

**The game has seven text fields.** That number is the deliverable, and it is exhaustive
rather than a survey, because there is a single flag that decides whether any keystroke lands:
`g_editActive` (`0x005AEB78`). `Screen_HandleInput` **clears it at the top of every frame** and
exactly **seven** of its arms set it again. Enumerate the writers of that one byte and you have
enumerated typing:

| # | screen | field | limits | destination |
|---:|---|---|---|---|
| 1 | `0x1F` page 4 | **the lord's name** | 16 chars, 192 px, kind 0 | `g_options` + 0, 31 bytes |
| 2 | `0x35` / `0x36` | the saved game's file name | 8 chars, 160 px, **kind 1** | `DAT_004EA130`, 64 |
| 3 | `0x1F` page 3 | the front end's load box, same field | 8, 160, kind 1 | `DAT_004EA130`, 64 |
| 4 | `0x1A` kinds 1–4 | **the letter you write to a lord** | 200 chars, *no* pixel limit | `g_diploLetterDraft + (kind−1)·200`, 199 |
| 5 | `0x1F` page 8 | the multiplayer game's name | 64, 192, kind 0, then lower-cased | `DAT_00553E80`, 64 |
| 6 | `0x1F` page 11 | the same on the skirmish page | as above | `DAT_00553E80`, 64 |
| 7 | *any* | **the multiplayer chat line** | 64 chars, 470 px, kind 0 | `DAT_00553E80`, 64 |

A **second enumeration from a different direction agrees**, which is the only reason to
believe the first. `Edit_Commit` (`0x0040210C`) is the *other* end — where the buffer is copied
out to wherever the field actually belongs — and it has **exactly seven call sites, which are
the same seven places**, each two lines below its own `g_editActive = 1`. Four distinct
destination buffers between them, because three of the seven share `DAT_00553E80`. Two of the
seven are built now (1 and 2); three of the remaining five are multiplayer, one is
diplomacy's, and one is a front-end screen still in the shell table.

**This paragraph said something weaker and wrong until it was re-read at the moment of
writing**, which is the practice `docs/agents.md` argues for and this is a small vindication of
it. The draft cited `Edit_Begin` (`0x00402009`) instead: *"23 call sites naming six distinct
destination buffers."* Both halves are wrong. `Edit_Begin`'s first argument is the **seed** —
the text a field opens *with* — not the destination, and its 23 sites name **fourteen** seeds,
most of which are string literals in `.rdata` rather than fields. Counting seeds would have
over-counted the fields and called it corroboration. The true second enumeration is the commit
side, and it is *stronger* than the claim it replaced: not "every one of them is one of the
seven" but **seven and seven, paired**.

**Why the arms audit found none of them, which is the part worth carrying.** C61 enumerated
`Screen_FrameInput` screen by screen and reported 80 of 185 arms — *and it counted mouse arms*,
because that is what `Screen_FrameInput` holds. **The keyboard is dispatched from the window
procedure** (`0x004B29BE`), which is not a screen ladder and was not in the audit's scope.
`WM_CHAR` (`0x102`) is one statement — `Edit_TypeChar(ch)` — with **no test of `g_screenId`
whatever, and the seven editing keys are equally ungated `WM_KEYDOWN` cases.** An audit that
reads per-screen ladders is structurally blind to input that is not per-screen, and this was a
whole class of it: *seven fields and eight keys, none of which any screen mentions.*

That generalises past this feature. **When an enumeration reports a percentage, ask what the
denominator's shape excludes**, not only whether it counted its own members correctly. C61's
denominator was "arms of `Screen_FrameInput`", which is a *place*, and every input that lives
somewhere else scored zero without ever appearing as a miss.

**What was found by reading the editor that would not have been guessed.** Sixteen functions
between `0x00401984` and `0x0040210C` are the whole of it, over one 2,000-byte buffer:

* **Overwrite is the default.** `g_editInsert` is BSS, zero is the overwrite branch, and
  `Edit_Begin` does not reset it — so typing `Ed` over the seeded `Player1` gives `Edayer1`.
  `docs/bugs.md` B79. Reproduced.
* **The character set is seven byte ranges and everything else is dropped in silence** —
  including the apostrophe, so `O'Neill` becomes `ONeill`. Kind 1 additionally refuses `,` `.`
  `?` `!` and lower-cases `A`–`Z`, which is a DOS 8.3 name.
* **The caret is the whole affordance.** There is no focus ring and no second plate frame; a
  field with no caret is indistinguishable from a label, which is exactly what ours were. It
  blinks eight ticks in seventeen and has two shapes, the opposite way round to the usual
  convention: underline for overwrite, I-beam for insert.
* **`VK_RETURN` is the save box's confirm button** (`Edit_Confirm`, `0x00401C5B`, sets the same
  latch the tick widget does). Ours had that as an unattributed convenience; it turns out to be
  an arm — the one guess in this area that was right.
* **`VK_END` cancels a confirmed save**, because it also calls `Chat_Close`, which clears that
  latch. `docs/bugs.md` B80.
* **The front end has no keyboard at all.** Not one of `Screen_HandleInput`'s thirteen
  `g_setupPage` arms tests a key, and the window procedure has no `0x1F` case. Our seven
  keyboard arms there are inventions; they are **kept** — a menu nobody can drive from the
  keyboard is worse, not more faithful — and recorded as `invention` with `removed: false`,
  which is the first use of that combination the schema was written for.

The inventory is `docs/arms.json`, groups `text` and `front-end-keys`, 29 new records; the
engine is `crates/l2-game/src/text.rs`.

**C75 — The field-coverage check matched prose, so the better a field was
documented the less it checked.**

`crates/l2-testkit/tests/encoding.rs` is the guard against `docs/decisions.md` C30's family —
*a field the encoder never writes*. Adding `Game::player_names` and then **deleting the loop
that encodes it** left the check **green**. The comment above the deleted loop still said the
words `player_names`, and `mentions()` matches text.

This is `docs/agents.md`'s *"a check that passes for an accidental reason"* with an
uncomfortable twist: the accident was **good documentation**. A field with a bare
`out.raw(...)` was genuinely checked; a field with a paragraph above it explaining what it is
was not. The better this project writes, the less that check was worth — which is exactly
backwards, and no amount of re-reading the check's assertions would have shown it. Only the
ablation did.

Two more holes fell out of the same half-hour, and both are the same shape — *a thing the
scanner cannot see is a thing it silently makes no claim about*:

* **`Game` was outside the check entirely.** It is the type at the top of every saved file, and
  the scanner keys on `impl Encode for T`; `l2_game::save` uses free functions
  (`encode`/`encode_prefix`), so `Game` had never been checked at all. It is named explicitly
  now in `FREE_FUNCTION_CODECS`, which **asserts that its heads still resolve** — a check that
  silently stops checking is worse than no check. Bringing it in required six deliberate
  `not-encoded:` markers, which is six decisions that were previously implicit.
* **`pub(crate)` fields were skipped.** The scanner stripped `pub ` only, so `pub(crate) turn:`
  produced a "name" with a parenthesis in it and was dropped by the identifier guard. One field
  invisible for as long as nobody looked.

**The rule this argues for**, and it is cheap: **a source-scanning check must strip what it is
not reading.** Comments are not code. And when such a check is extended to a new type, run the
ablation *for that type* — the general "does the check work" was answered years of tests ago
and says nothing about whether it works here.

**C76 — the county sidebar, the menu bar, and three arms that were wrong rather than missing**

The input audit put the campaign-side chrome — the right column, the menu bar, the four
county panels and screens `0x04` … `0x13` — at **64 of 114** arms. This is the pass on it,
and the shape of what was found is the finding rather than the count.

**A one-line "not reproduced" in a table hid twenty-one arms.**
`crates/l2-game/src/screens/map.rs`'s header carried three rows reading *"**not
reproduced:** the menu bar's three titles — `Menu_OpenDropdown`"*, *"…right release clears
the minimap mode"* and *"…the sidebar's job rows"*. The first of those three lines is
**sixteen menu items over three drop-downs plus four dispatch arms**, because
`g_menuBarItems` (`0x004DC428`) is three 16-byte records pointing at three item tables and
screen `0x32` has an arm of its own. A row of a table is a unit of *writing*, not a unit of
*work*, and nothing in the project could see the difference until `docs/arms.json` counted
one record per arm.

**Three arms were WRONG rather than absent, and that is the number nobody had.**

1. **A right click while carrying peasants left the village.** `0x06`'s arm is
   `g_screenId = 0x02; g_villageDragCluster = 0;` — *put them back and stay*. Ours popped
   the screen from every phase, so a player who changed his mind lost the village with the
   selection. It looked right because leaving on a right click is exactly what the *idle*
   village does.
2. **The farm/industry slider's up zone started one pixel late**, `x > 594` against the
   original's `x >= 594`, so one column of the track did the wrong thing; and it had **no
   ownership gate at all**, where `Labour_SplitSliderDrag`'s second line is
   `if (county.owner == g_localPlayer)`. Ours moved another lord's peasants.
3. **A left click anywhere closed a shell.** All twenty-six close tests in
   `Screen_FrameInput` are `Ui_OkButtonClicked`'s 24 × 24 corner box; there is no screen in
   the game where clicking the middle of a panel dismisses it. Screen `0x04` — the
   information panel a right click on the map opens — could not be *read*, because the
   click that opened it was followed by the click that closed it.

**And one control of ours was painted on top of one of the game's.** The county panels drew
a BACK TO MAP button at (478, 460), 162 × 20, and hit-tested it first. That rectangle is
`g_sidebarButtons` record 5 to the pixel — **End Turn**. It is the same defect a player
reported a fortnight ago about the five sidebar icons, made again, in the same column, by a
different screen. Removed.

**The county panels swallowed the whole right-hand column**, which is the largest single
gap. `Screen_FrameInput`'s arms for `0x14`, `0x15`, `0x16` and `0x19` open with the *same
six guards* the village's arm opens with, every one of them hit-testing `x >= 0x1DE`, and
every one of them **ahead of the panel's own two ways out**. The village had them
(`Transition::Pass`, C59) and the panels did not, so with a panel open the minimap, the five
sidebar buttons, the split slider, the produce rows and End Turn were all dead. One shared
predicate now answers for both.

**`Transition::Reveal`, and the arm that needed it.** `Screen_FrameInput` has an *epilogue*
that runs after every per-screen arm on every screen id but `0x12`:
`if ((leftPressed || rightPressed) && FUN_004323FE()) { if (g_screenId == 0x0F)
Sound_StopOneShot(); if (g_battlePhase == 0) g_screenId = 0; }`. So **the minimap is not the
campaign map's control, it is the game's**: a press on the raster from any management screen
re-centres the map and drops the whole surface. Our stack had no way to say *"I acted and
everything above me closes"* — `Push`, `Pop` and `Replace` all move the acting screen — so
that is a fifth transition, and it is `g_screenId = 0` with a stack underneath rather than a
convenience. `Minimap_Click` refusing screens `0x05` and `0x06` outright is its own guard and
is why a peasant drag cannot be lost to a stray click.

**Two visual defects fell out of making the column live.** Opening a county panel repainted
the seven `Misc_cty` column plates over the map's, which **blanked the minimap** — harmless
while the column was dead, a control you can click and cannot see now that it is not. And
the menu bar's three titles start at x = 10, where our TURN and COUNTIES lines were: two
lines of ours were sitting on the way into every menu in the game, invisible with the real
fonts loaded because `Ui_DrawMenuTitles` measures the captions and ours were drawn first.
Both are the same mistake as the BACK TO MAP button and all three were found by making the
original's control work, not by looking at the screen.

**Where the denominator is wrong, and it is not wrong in our favour.** C61 counted *"the
arms of `Screen_FrameInput`"*, which is a **place**. Every input the game dispatches from
somewhere else scored zero without ever appearing as a miss. Three such places are now known
and each was found by somebody looking outside the dispatcher: the **window procedure**,
where the whole keyboard lives ungated by `g_screenId`; **`Screen_HandleInput`**, whose
`0x0F` arm holds six hotspots that appear only when the job is the blacksmith and are *the
only control on any job popup* (`0x004BA9C8/blacksmith-weapon-choice`); and
**`Screen_DrawWidgets`**, the draw pass, where hover lives — and where
`Ui_DrawMenuTitles` writes the menu bar's own hit boxes back into its table, so the bar
cannot be hit-tested until it has been painted. `docs/arms.json`'s `_note` now says this
where the number is taken.

**A stray NUL byte in `crates/l2-game/src/screens/map.rs`** — a `'\0'` pasted verbatim out
of the decompiler into a doc comment — made `grep` treat the file as binary and hide every
`// arm:` marker in it from a plain search. `rustc` accepted it and so did the arms test,
which reads the file with `read_to_string`; only the human-facing tool lied. Replaced with
the two characters it was meant to be.

**And the keyed merge driver guards the array and not the scalars beside it.** Rebasing this
work onto `main` merged `docs/arms.json`'s `arms` array by `id` exactly as intended — *"114
arms, no entry changed on both sides"*, and it was right — and then took the other side's
`groups` object and `_note` wholesale, **discarding five group declarations and twenty-one
lines of prose that existed on only one side**. Every test in `crates/l2-game/tests/arms.rs`
stayed green, because every one of them reads the `arms` array and nothing else. That is the
same shape as the `addr`-versus-`id` key defect the driver was hardened against an hour
earlier, one level up: the *entries* are keyed and the *object they sit in* is not. It was
caught only because the group count was read by hand afterwards.

**What `xref.js` settles, now that it runs from a worktree.** Two exhaustiveness questions
that an enumeration of one dispatcher cannot answer:

* **`g_mouseRightReleased` has exactly eight readers in the corpus**, and on the campaign-side
  chrome only three of them matter: `Screen_FrameInput`, `FUN_0047685D` (the message scroll)
  and `FUN_00439079` (the minimap overlay). `Battle_UnitPanelClicked` is the battlefield's;
  `FUN_004B191E` is the frame poll that *computes* the flag; and `FUN_0040E680`,
  `FUN_004B1F4A` and `FUN_004B1FEC` are all the same thing — **modal spin loops** that wait
  for any button to come up, used for splash pauses. So *"the right button lives in
  `Screen_FrameInput`"* is now checked rather than assumed for this group, which is the one
  claim the audit's second pattern rests on.
* **`Hotspot_Test` has nine callers and `Widget_Test` four.** Every one is accounted for
  except `FUN_00437107` (`0x00437107`, 199 bytes), which tests a two-record table at
  `0x004DC5F0` plus a rect at (16, 400, 240, 64) for the selected army — **and has no caller
  at all in the corpus.** It is not filed in `docs/arms.json`, because "xref found no caller"
  is a failure to find and not the exhaustive check `dead` requires; it is written down here
  as a lead, and it is the shape of a second unit panel.

None of the twenty-one `FUN_` addresses this pass rests on is named in `docs/symbols.json`
even after the 211-name refresh, so every one of them was read as a body rather than trusted
as a name. They are the obvious next batch for the naming campaign: `FUN_0040DD92`,
`FUN_0040DF62`, `FUN_0040E099`, `FUN_0040FEC1`, `FUN_00437002`, `FUN_00438990`,
`FUN_00438A91`, `FUN_00438B02`, `FUN_00439079`, `FUN_0043A950`, `FUN_0043A997`,
`FUN_0043B412`, `FUN_0043B4CB`, `FUN_004323FE`, `FUN_0043CAF4` and `FUN_0047685D`.

`every_group_an_arm_names_is_declared` closes the specific hole in both directions — an arm
filed under a group nobody declared, and a group nobody files under — and it was ablated by
deleting one declaration. The general hole is the driver's and is left for whoever owns it;
the file's own `_note` now tells the next person to check `groups` and `_note` by hand after
any merge. A `group` carries the owner and the `complete` flag, so an arm whose group has
quietly vanished is precisely the arm that stops being counted, in the file whose entire
purpose is counting.

**C77 — The picture on a pasture is the herd count, and we had reproduced the meter
and not the picture.**

A player with the build in front of him: *"why do the pastures not have cows in them?"*

Because `FUN_004071A0` — the overlay pass `Map_DrawFrame` runs between the terrain and the
army sprites, on runtime bank bit `0x80` — has four arms and we had built three. The town's
flag, the castle's garrison banner and the mercenary marker were drawn. The fourth is
farmland, and it is the animals.

The chain is short and every link of it was already in this repository, unjoined:

* `Terrain_Set` sets bank bit `0x80` when `0x0E < terrain < 0x17`. That range is *exactly*
  the pasture range, so **a pasture is the only field state that gets a second blit at all**
  — and `l2_view::campaign::field_graphic` already carried the bit, with a comment guessing
  *"on a pasture, presumably the animals"*. It is.
* The arm picks `Flags1a.pl8` frame `0x55 + (terrain − 0x14) * 6 + phase` for a stocked
  pasture, at tile origin `(+4, −4)`, and returns without drawing for `0x13`.
* `Herd_UpdateCrowding` (`0x0044D913`) is what writes that terrain, onto **every** pasture
  tile of the county, from `herd / fieldsCattle` banded at 11 and 21.

So it is a **rule wearing a graphic's clothes**, and the shape of the miss is the one C61
named: we had `land::herd_crowding`, the *meter*, reproduced and tested; we did not have
`Herd_UpdateCrowding`'s other half, which is the same function.

**The bands are not the same bands, and that is the trap.** The meter has four —
`< 11 → 10`, `< 21 → 20`, `< 31 → 30`, else 40 — and the picture has three, merging 30 and
40. A renderer that indexed three pictures by `herd_crowding / 10` would run off the end of
its table **on county 1 of the shipped England position**, which grazes 74 head on one field.
`land::herd_graphic` is therefore its own function beside `land::herd_crowding` rather than
a lookup on it, and `the_map_merges_the_top_two_crowding_bands_and_the_meter_does_not` is
that difference asserted.

**The evidence is the original's own save, on fourteen counties, and it is the strongest
shape this project has.** The England turn-one fixture stores `county.herd`, it stores
`fields_cattle`'s twenty tiles, and it stores the terrain byte the original wrote on each —
so reproducing the third from the first two is a check nothing in this tree can make come out
right by agreeing with itself. 107 pasture tiles, three of the four states, every one exact.
That also means the fixture *already carried* the right pictures: the only thing missing was
the drawing, which is why nobody noticed the rule was missing either.

**Two things about the artwork that a canvas diff could not have told us.** Frames
`0x55 … 0x66` are eighteen frames of **58 × 30** — exactly the near-zoom diamond, so the
sprite is a full-meadow overlay — and their opaque pixel counts run 344 / 706 / 930 across
the three bands, which is *more animals on a more crowded meadow* stated as something the
file could have contradicted. The second block the ladder can reach, `0x67 … 0x78`, is
eighteen **2 × 2 stubs**: art that was reserved and never drawn.

**And that second block is not sheep.** The natural reading of a second three-group ladder is
a second species. The tile-info table at `0x004D2EC8` refutes it: content `0x0F … 0x12` gets
`L2.eng` group 30 descriptions 40 … 43 and mode 20, which are *the same four strings* as
`0x19 … 0x1C`, the live reclamation ladder. It is a vestigial earlier encoding of field
reclamation whose art moved with it. Nothing writes those four values onto a farm tile in the
shipped game — every `Terrain_Set` call site was enumerated — so the arm is dead, and it is
reproduced rather than dropped because the ladder is what the function does.

There are **no sheep anywhere**: a pasture is a *"dairy meadow"* and its mode line is
*"- Cattle."*; no sheet holds a sheep; no `farm_style` branch picks a species.

**The clock is not the village's.** The natural guess — and the one the brief made — is that
the animals take a pulse from `Tick_Pulses` (`0x004BBC80`), the 20 ms `timeGetTime` divider
chain the village animates off. They do not. The campaign map has its own clock,
`FUN_004CFB08`, a **16 ms `GetTickCount`** gate stepping two counters: `DAT_0057D378`,
wrapped at `0x80`, whose `>> 4` is the flag's eight wave phases, and `DAT_0057D388`, wrapped
at **`0x60`** — not a power of two — whose `>> 4` is the herd's six. A herd holds each frame
for 256 ms and the loop takes 1.54 seconds.

**C78 — Emptying the shell table found four wrong claims, and the table's shape is
why they survived.**

Seven rows were left. All seven are gone. Six graduated into modules; the seventh was never a
shell at all.

| row | the claim | what it was |
|---|---|---|
| `0x0F` *"The job popup"* | name, painter and group all **correct** | **a live duplicate of `screens/job.rs`**, which had claimed the same id and painter since it graduated. Two index entries for one screen. |
| `0x04` *"The map information panel"* | *"`FUN_0041B032` draws no `Ui_DrawBox`"* | **both** its halves open with one. Somebody read the dispatcher's own 79 bytes — which contain no drawing at all — and concluded the callees did not either. |
| `0x09` *"The court"* | four lines in the `lines` field, *"the body lines, in the 14-pixel font"* | the painter draws all four in the **22-pixel heading font**, and the fifth entry is a **button caption**, not a line. |
| `0x2E` *"the seven rating rows per player"* | seven rows | **seven columns by three rows**, twice. The columns are troop types and the rows are Before / Killed / Kills. |

`0x0F` is the instructive one. **Nothing could have caught it**, because the only check —
`every_shell_has_a_distinct_screen_id_and_a_painter` — dedupped *within* the table, and
`find` was only ever asked about ids the table already held. The check that would have caught
it is the opposite one: *a shell id must not be a screen id we build*. That is now
`shells.rs`'s `find_answers_for_every_id_that_ever_sat_here`, and it is C61's shape again —
a rule that reads as complete while naming no artefact the failure could live in.

**The general lesson, and it is why `shells.rs` survives as a record with an empty array: a
row of five fields cannot say what a painter does.** Every graduation found something the row
had no place to hold — that `0x0A` is where an army is *created* and not a shop, that
`0x18`'s widget table carries a complete **sheep row** the game never passes, that `0x25`'s
apologetic *"nothing — this is the whole screen"* was a **measurement** of a 171-byte
function. A module header can hold a painter as a literal listing; a table cannot, and four
weeks of wrong priorities came out of the difference.


**C79 — `FUN_00496B9F` is the garrison lowering its drawbridge, and
the hand-off called it siege-engine placement.**

The brief this work was done from described `FUN_00496B9F` as *"siege-engine placement, and
it is the thing to implement"*, from a correct reading of what the function *writes*: a 7 × 4
patch over the first `flags & 0x40` cell, surface 3, frames from `DAT_004D9E18`, four on both
siege scores, pathfinding rebuilt. Every one of those is true. The verb was wrong, and four
independent sources say so:

* the button that calls it, `FUN_0043BBE7`, has three refusals and the game wrote all three —
  `L2.eng` group 110 *"Sieges only!"*, group **111 *"No drawbridge!"*** and group **157
  *"Drawbridge is down."***;
* its guard is `g_castleLevel < 3`, and the shipped `Readme.txt` says *"only the Stone and
  Royal castles have drawbridges"* — the same two castles;
* the Readme also says *"within a siege, drawbridges can not be closed once they have been
  opened"*, which is `DAT_0052AF9C`, the one-shot latch the routine sets;
* the routine sets `_DAT_00569588` and adds 4 to both scores, which are **exactly** the three
  writes `BattleMan_StateRamGate` makes on the twenty-thousandth gate hit. The garrison
  opening its own gate is indistinguishable, to the besieger's AI, from the besieger breaking
  it — which is what makes the Readme's sentence a rule rather than an interface quirk.

`crates/l2-sim/src/siege.rs` already read `0x40` as the drawbridge and cited the Readme for
it; the hand-off and the code disagreed and nothing compared them. The lesson is not that a
decompiler reading was wrong — it was right about every byte — but that **naming a verb from
what a function writes, without asking what the game calls it, is a different act from
reading it**, and `L2.eng` is the cheapest check there is. `docs/formats/eng.md` §5 is
indexed by group for exactly this, and the answer was two lines of it.

**C81 — the moat-fill state had no writer, because the flag was read off
the wrong cell.**

`State::FillingMoat` was in `l2-sim` from the start, `crate::ai`'s `Order_ToBreachOrStaging`
ordered units at the ditch, and **not one figure ever entered the state**, in any battle, by
any route. `Formation_SendFigure` tested `cell.surface == 2` on the **slot it was sending the
figure to** — which reads like the same thing as the original's `DAT_00553FE4` and is not. A
slot is chosen either by the formation rectangle or by `Formation_SlotIsUsable`, and *both of
them reject an impassable empty cell*, which every moat cell is. The water branch was
unreachable by construction.

The original caches the flag in `Formation_RectIsClear`, off the **unit's ordered
destination**, and then rejects the rectangle for the same reason — so a unit sent at the
ditch has *every* figure enter state 9 while walking to a **dry** slot beside it, and
`BattleMan_Step`'s state-9 arm latches whatever impassable cell stops it.

Two more halves were missing with it, and each alone was enough to keep the state inert:

* **the handler's tail.** `FUN_004926FB` retargets a figure that has finished a cell at the
  nearest water within nineteen, and only when there is none left does it drop to state 5.
  Without it a man stood on the cell he had just filled for the rest of the battle.
* **the reform gate.** `BattleUnit_Order` sets `g_battleUnits[unit].field_0x13 = 1` on every
  order, and `Formation_SendFigure` refuses to re-issue to a state-9 figure *unless* it is
  set. Nothing here set it, so a man ordered to fill a ditch could never be ordered to do
  anything else again.

This is C27's shape three times over in one feature, and the reason none of the three showed
up is the same: **nothing had ever fought a siege to its end.** The `l2-sim` tests ran 600
frames to enumerate handlers; the seam tests asserted that a result came back. The first test
that watched one for sixty thousand frames found all three in an afternoon, plus
`C82` below.

**C82 — the third way a siege can end could not be reached, and the
elevation was ours.**

`crates/l2-sim/src/siege.rs`'s `our_castle` put the `FLAG_KEEP` cell at elevation 3 in a
bailey of elevation 1. `Formation_RectIsClear` rejects a rectangle whose slots are not *at*
the destination's elevation and `Formation_SlotIsUsable` rejects a slot more than one below
it, so an order onto that cell was an order no figure was ever given: the besiegers walked
into the bailey, found nowhere to stand and stopped. `Battle_CheckOutcome`'s third arm —
*"getting one man to the keep's door ends the siege"* — was documented, implemented, and
unreachable.

The elevation was invented here rather than read, so it is corrected here rather than
catalogued in `docs/bugs.md`. Two neighbouring numbers were the same kind of thing and both
are cited now: a breach is left at `BREACH_ELEVATION` = 0 because `Wall_Collapse`
(`0x0047DFE0`) writes `elevation = 0`, and the gate-breach arm had been leaving it at the
wall's own height — a hole in a two-high wall that a man on the ground cannot step through,
because `can_step_elevation` allows one.

**C80 — a player besieged by an AI could not end his turn.**

`turn::question_for` computed `g_battleChoiceOwner` as a paraphrase of
`Battle_ChooseSettlement` (`0x004A6A30`), and the paraphrase tested the **defender's**
`ownerIsHuman` where the original tests the **attacker's**. The original is two `if`s and the
second overrides the first:

```c
if (units[armyA].owner == localPlayer) choiceOwner = 1;
if (units[armyB].owner == localPlayer) choiceOwner = units[armyA].ownerIsHuman ? 2 : 1;
```

— *a human defender attacked by an AI holds the choice himself*, and only two humans put it
in the other man's hands. Ours handed that defender a **2**, which is the *"your opponent has
the choice"* notice, and `Battle_ChooseSettlement` writes `DAT_00554408 = 2` — the widget
count — only under 1. So the prompt came up with **no buttons**. In the original a
bystander's prompt waits for the multiplayer answer timeout; single player has no such
timeout, and ours has no timeout at all. The turn was suspended, two armies stood on one
tile, and there was no input that would move the game on.

It was unreachable until phase 2 could raise a siege prompt, which landed the same week — the
defect and its route arrived from different directions and neither agent could see the
other's half. It is also the gate on every garrison verb there is: the drawbridge button
lives on the battlefield, and until this was right no besieged human could reach the
battlefield.

**C84 — Building the missing writer closed C68's gap and then a *second* one
opened underneath it, and the test written to catch the first could not see the second.**

C68 named four fields with one writer each and no writer, and predicted that the day
`l2_kingdom::diplomacy` landed, `tests/ai_war.rs`'s first assertion would go red and say so.
It did, exactly as designed. **And the raid still could not fire**, because forty turns of
England then produced *no standing below −10 anywhere on the map* — every AI-to-AI pair
saturated at **+30**, `AI_Diplomacy`'s heal of one a turn having nothing to work against.

The reason is that the module was only half of what was missing. `Diplo_Offend` is the single
hook every relationship-damaging act goes through, and its four call sites are **not in the
diplomacy code at all** — they are in the mover (`Unit_CrossField` `0x0046673C`,
`Unit_BurnDwelling` `0x00468AE2`), in the transport collision, and in
`Battle_ReturnToCampaign` (`0x004AB383`). Two of them already existed here as *reported
values* — `movement::Offence` and `battle::Aftermath::offence`, both with doc comments saying
*"for a caller that has a diplomacy layer to drive"* — and nothing was driving them. Wiring
those three lines is what turned the numbers from *20 standings, 0 below −10, 0 wars* into
*21 standings, 4 below −10, 1 war, 1 war target* on the same forty turns.

**The lesson is about the shape of the test, not about the wiring.** C68's test asked *"has
anything written a standing?"* — and the answer became yes the moment `Diplo_Init` ran, which
happens before turn one and proves nothing about play. The replacement asks two questions
that each name a mechanism: *is any standing above `Diplo_Init`'s opening 5* (only the heal
can do that) and *is any realm allied* (only the courtship can). Both were run as ablations:
deleting the step-2 dispatch turns them red, and — the point — **the original assertion stayed
green with step 2 deleted**, because the battle hook alone satisfied it. A gap-closed test
inherits the gap's own framing, and that framing is *"is the field non-zero"*, which is the
weakest question in the family.

The general form, which is worth more than the instance: **a test written to go red when a
gap closes should be replaced, not merely satisfied.** Its job ends the moment it fires, and
what it leaves behind is an assertion tuned to the absence rather than to the behaviour.

**C85 — An army was allowed to reinforce itself, and it took a use-after-free to
find out.**

`Army_GarrisonApply` reaches `Army_Combine(sitting, army)` with **no test that the two are
different slots**, and `Army_Combine` has none either: it would sum a garrison's men with its
own and then free the record it had just doubled. It is unreachable from the original's map,
because an army already sitting in a castle has no orders to give.

It became reachable here the moment diplomacy started aiming armies, and it arrived as a
panic in `unit::combine` — `units.remove(from)` followed by `get_mut(into)` on the slot just
emptied. **Refused rather than reproduced**, in `conquest::reach_castle_building` and
`Kingdom::garrison_army`: reproducing this one means reproducing a use-after-free, and the
behaviour it would produce in the original is not a rule anybody could have observed.

Worth recording for one reason beyond the fix: **it was found by a behaviour change three
subsystems away.** Nothing about diplomacy touches garrisons. What diplomacy changed was
which county an AI marches on, and one of those marches happened to end on a castle its own
army was already in. A latent guard is only latent until something upstream of it moves.

**C83 — Loading a mid-game save reset every alliance and every grudge, and the fixture that
could have caught it was the one fixture that cannot.**

`l2_formats::save::Realm` did not read `+0x84 … +0xE3` at all. Ninety-six bytes a save,
five hundred and seventy-six across the six realms: the whole diplomatic matrix, in the file,
dropped on the floor. `scenario::from_save` then ran `Diplo_Init` to fill the gap.

That is **exactly right for the England turn-one fixture** — `Game_NewGame` runs `Diplo_Init`
and nothing has moved by turn one — and wrong for every save after it. A player loading a
mid-game file got an AI that had forgotten the war it was fighting.

**The seventh instance of a field nothing writes, and the first found because somebody built
the consumer.** The other six were found by asking *"who writes this?"* and getting no answer.
Here the field had a reader all along — `l2_kingdom::realm::Pair` is documented, offset by
offset, and the diplomacy rules read it every turn — and the question nobody had asked was
*"who fills it on the way in?"* Building the diplomacy screen made the emptiness visible,
because an empty matrix is invisible until something draws it.

### The fixture could not have failed, and that is the part to carry

The obvious test is to load the England fixture and assert the standings survive. It passes
either way. `Diplo_Init` opens an in-play AI realm at **5**, and England turn one **is** 5
everywhere — so the correct import and the missing one produce identical output on the only
fixture the suite reaches for by default.

The saves the player made are the oracle, and they can tell the difference:

| save | realm 2's standing toward realm 3 |
|---|--:|
| `england-turn1.sav` | 5 — `Diplo_Init`'s opening value |
| `safeturn.sav` | 7 |
| `old_turn.sav` | 8 |
| `battle-after.sav` | 10 |
| `siege-lastturn.sav` | **18** |

Thirteen turns of the +1-a-turn heal above the opening 5, and `Diplo_Init` would put every one
of them back to 5. So `a_mid_game_save_keeps_the_standing_it_was_saved_with` asserts **18**, and
ablating the carry gives 0 rather than 18.

**This is the same shape as C61's denominator and §2.10's cattle**: a check that cannot
distinguish the two answers is not a weak check, it is not a check. *"We assert this against the
fixture"* is worth nothing until somebody asks what the fixture would look like if the claim
were false. Here the answer was *"identical"*, and the only reason it was noticed is that the
instruction to land this said **mark it `[I]` if no fixture can fail** — which forced the
question before the test was written rather than after.

### And the shape came from the file, not from our model

The pair is **nine fields over sixteen bytes**, and the first reading of it was five: standing,
allied, grudge, warnings and a byte at `+0x04` guessed as *"refused"*. `+0x04` is `at_war`, and
`+0x05`, `+0x08` and `+0x0C`–`+0x0D` carry the compliment count, the best gift ever received and
the mail flag with the help-price multiplier. Reading five and stopping would have carried the
standing — the visible thing, the thing the test asserts — and **silently dropped the gift
history**, which is exactly the failure being corrected, one level in.

The compiler caught it, because `l2_kingdom::realm::Pair` has all nine and a struct literal must
fill every one. That is the exhaustive-literal discipline paying for itself on the first field
added after it landed.

### What the exhaustive destructure did and did not do

`RealmState` is destructured with no `..` at the consumer, so adding `pairs` produced **three**
compile errors — both producers and the consumer — and none of them could be forgotten.

But it did not *find* the hole, and the reason belongs beside the destructure itself: **a
completeness check over a struct is only as complete as the struct.** `l2_formats::save::Realm`
had no `pairs` field to omit. The destructure protects the fields we know about and says nothing
about the ones we never modelled — which is the same sentence as C61's denominator being a
*place*, and as *a check on existence is not a check on meaning*.

**C86 — the original has a generic tooltip layer, and `docs/screens.md` said it
did not.**

`docs/screens.md` §7 ended a paragraph about the merchant's price plaque with *"**this
engine has no generic tooltip mechanism**, and `0x00553ECC`, the only candidate on file,
turned out to be a click guard."* It meant *our* engine. It reads as a claim about
`Lords2.exe`, and about `Lords2.exe` it is false: `FUN_00476E95` runs every frame from
`Battle_Frame`, gated on `g_optToolTips`, waits a second of `timeGetTime` with the pointer
still, resolves a hotspot id through the per-screen table `DAT_004D6FB8[g_screenId]` and
draws `L2.eng` group 220 index *id* beside the cursor. `docs/formats/eng.md` §5 has had it
right, `[V]`, the whole time — **two documents, one wrong, and nobody had reason to read
them together.**

**Twenty-four of the thirty-five strings are the campaign sidebar**, resolved by
`FUN_00477320` (1,082 bytes). They name the five sidebar buttons in order and every produce
row in order — an independent confirmation of `map.rs`'s `SIDEBAR_BUTTONS` table and of
`FUN_0040FEC1`'s two lists, arrived at from a completely different direction. That is the
argument for the enumeration: *the game had written a description of the screen we were
reverse-engineering by hand, in the file we already read for everything else.*
`docs/draws-map.md` §5.1.

**C87 — only the town arm guards a zero shield, and one document said both
did.**

`Sprite_TopIt`'s town arm returns on `county.field_0x7 == '\0'`. Its castle arm tests
`garrisonUnit == 0` and then computes `shield * 8 - 8 + phase` with **no clamp anywhere in
the function** — the `1 … 5` clamp `docs/screens.md` §5.1 credited it with is
`FUN_004171EE`'s, on the menu bar's banners, a different function on a different sheet. A
zero-shield garrison therefore asks for frame `−8 + phase`; the frame record fails the
`dataOffset < 1` check and the function writes `"ERR:top_it no data"` and sets
`g_quitRequest = 1`. **[D]** — no shipped save on this machine has one, so it is read and
not observed. The shape is C21's: a clamp seen in one place and attributed to another.

**C88 — every army and every mob carries a banner, and the section describing
the unit sprites did not mention it.**

`docs/screens.md` §5.2 gives `Map_DrawArmies`' sheet, frame arithmetic, anchor, per-kind
nudge and walk tables. Still inside the same loop and after `x -= w/2; y -= h`, an army
(kind 1) with a non-zero shield gets `Flags1a` frame `(shield−1)*8 + phase` at `(+0x12,
−0x15)` and a peasant mob (kind 2) gets `0x79 + phase` at `(+0x12, −0x12)`; merchants and
transports get none. So **`Flags1a.pl8` frames `0x79 … 0x80` are the mob's banner** — eight
`16 × 42` standards, measured against the player's own file, in the block
`maps-layers.md` §5.5a had left unnamed after the `2 × 2` stubs.

**And the first draft of that measurement was wrong in the way this project's measurements
usually are.** The test asserted `32 × 24`, the size of the realm flags at the head of the
same sheet — a size assumed from a neighbour rather than read. It went red on its first run.
That is the ablation rule paying out on a check whose subject cannot be ablated: *make it
fail once and read what it says.* `docs/draws-map.md` §5.3.

**C89 — the words in the far-zoom strip are the original's, and we
replaced them with our own.**

`docs/screens.md` §7 lists the far zoom's `Ui_DrawBox(0, 412, 30, 4)` among what we
reproduce and says *"the box is the original's; the words in it are ours."* The words are
the original's too: `Screen_DrawCampaign` puts four things in it — `L2.eng` group 101 at
`g_scenarioIndex` (the map's name), group 34/0 (*"Year"*), `Ui_DrawYear`, and group 34/1,
***"Click on the county you wish to view."*** So the far view is the game telling the player
in its own words what the far zoom is for, which is also why `Map_Click`'s whole dispatcher
sits inside `if (g_mapZoom != 2)`. Ours draws a status line of its own there instead.

Cheap to close, and worth closing first of the four: it is the only one of this audit's
findings that needs no fixture the project does not have. `docs/draws-map.md` §5.4.

**C90 — Twenty revolts in a hundred turns, and every one of them was a message and
nothing else.**

`docs/plan.md` §2.5 asked for the hundred-turn game because *"every rule that fires only
when a realm holds more than one county has no oracle at all"*. The first thing it found was
not a multi-county rule at all. It was that **`County_RaiseRevolt` (`0x004AC185`) is not
implemented**, and had never been: `crates/l2-kingdom/src/unrest.rs` reset the counter,
pushed a `Message::Revolt`, and returned — with a comment saying *"raising the mob is a unit
operation and therefore not this crate's"*, addressed to a caller that did not exist.

**What that cost, measured rather than argued.** A hundred turns of the England fixture, the
unplayed human realm left to starve as the control:

| | before | after |
|---|---|---|
| the person's realm | **in play on turn 100**, one county, population **0** since turn 22 | eliminated on turn 24 |
| `g_gameOutcome` | never left `InPlay` | `Lost` on turn 24 |
| peasant mobs ever on the map | 0 | 67 sightings |
| battles in a hundred turns | 0 | 17 |
| counties changing hands | 0 | 2 |
| the largest realm | 3 counties | **7** |

The last row is the one that matters for §2.5: **a realm holding four or more counties is
now reachable in an ordinary England game, without a hand-dealt board.** It never was
before, and that is why §2.5 could describe the multi-county rules as having no oracle
*and* no way in.

**Reading the function properly cost four more corrections**, all `[V]` from
`Unrest_UpdateAll` (`0x0044AA41`) and all written up in `docs/kingdom.md` §6. Two of them
were tests of ours asserting the opposite of the binary, both sourced from prose rather than
from the function — the C58 shape again:

1. **AI-owned counties revolt too.** The call sits at `LAB_0044ADF3` *inside* the AI branch;
   the human branch reaches it with a `goto`. `unrest.rs` had a test called
   `an_ai_county_never_raises_a_mob`, citing `docs/kingdom.md` §6.
2. **It fires only on a season the counter went up** — `if (before < after)` on both
   branches. So a county whose mob could not be placed sits at 4 for ever.
3. **The warning season and the ladder season are exclusive.** `Msg_Enqueue(0x92)` is the
   `if` and the whole ladder is its `else`, so a revolt lands on the **fifth** season below
   25, not the fourth. The manual says *"more than four seasons"* — read whole, it agrees,
   and `docs/kingdom.md` §6 had taken *"more than"* to mean *"at least"*. That is
   `docs/agents.md`'s *citing an oracle is not reading it*, on the same sentence a second
   time.
4. **A human county's unrest counter is cleared outright at happiness 25.** `unrest.rs`
   asserted it was sticky and called the stickiness *"real in the documented rules"*. No
   document said it.

Three and four together make revolt considerably rarer — five *consecutive* seasons, not
five seasons spread over a reign — which is visible in the numbers: the same hundred turns
raises 2 revolts under the corrected ladder where it raised 20 under the old one.

**What generalises.** The old code's comment is the artefact worth keeping: *"not this
crate's"* named a real architectural boundary, was true when it was written, and became a
missing feature the moment nobody owned the other side of it. `docs/plan.md` §2.4 records
the same failure in `ai.rs` — *"reproducing them here would mean inventing a unit model"*,
written the day before `l2-kingdom` acquired a unit model. **A comment that defers work to a
caller should name the caller**, because a deferral with no addressee is indistinguishable
from a decision not to do it.

**C91 — `WEATHER_JITTER_BOUND` had been settled twice and the log still
said it was invented.**

`docs/plan.md` §2.7 calls it *"the one constant in `l2-kingdom` with no evidence behind
it"*, and the Open questions list below said the same. Both were stale:
`crates/l2-kingdom/src/weather.rs` has carried the derivation since the day it was traced —
`Rand_Advance` (`0x00404A46`) masks its LFSRs with `0x7F` and `Weather_UpdateAll` shifts by
3, so the draw is 0…127 and the jitter 0…15 — and `docs/audit-method.md` re-derived it
independently a second time and recorded that the decision log was stale. **The constant is
128 and it is right.** Re-read a third time here from the two functions, because a claim
believed on the strength of two agreeing summaries is C13's shape.

The interesting half is what the same paragraph *left* open and nobody chased:
**`localModifier` — `FUN_00449D6E` — returned zero in our code and was marked "never
traced".** It is a per-county term worth up to 12 a season on the dryness accumulator, and
weather drives sowing, growth, harvest and the herd every season for a hundred seasons,
which is §2.7's own argument for why it matters. It is traced now (`docs/kingdom.md` §7.3):
a climate band 0…4 cut out of the county's **index** by `County_Reset` (`0x00451150`), read
by nothing else, with a Summer ladder that skips band 3 and a dead `−24` arm
(`docs/bugs.md` B92).

**Two things to carry.** First: an open question is a claim about the present and goes stale
like any other — this one had been false for weeks in a list nobody re-reads, and the
mechanism that caught it was an agent being told to settle something already settled.
Second, and sharper: **the paragraph that was stale and the paragraph that was live were the
same paragraph.** A single entry closed the half that was easy to check and left the half
that was not, and the closed half is what everyone read. An open question that names two
things should be two entries.

**C92 — the third invented hotspot sitting on a live control, and the
test written to catch it named the wrong rectangle.**

`docs/decisions.md` already has two: a COUNTY PANEL button of ours over the five
`g_sidebarButtons` icons, which a player reported as *"the icons in the bottom right do
nothing and have text over them"*, and a BACK TO MAP button at (478, 460) 162 × 20 on the
four county panels, which is `g_sidebarButtons` **record 5 — End Turn** — to the pixel and
hit-tested first. The brief for this branch asked for a third. There is one.

`screens/divide.rs` drew three buttons of its own — SPLIT, DISBAND and CANCEL, 18 pixels
tall at y 446 — below the original's window. CANCEL is (264, 446) 100 × 18. `g_splitWidgets`
(`0x004DD388`) record 0 is **the confirm tick**, 32 × 32 at (288, 420). They overlap in a
32 × 6 strip, and ours was tested first: aiming at the bottom two rows of the game's *split*
got our *cancel*.

**The part worth carrying is not the overlap; it is the check.** That module had a test
named `our_own_buttons_do_not_sit_on_anything_the_painter_drew`, and its body is

```rust
assert!(r.x + r.w <= OK.x, "{r:?} runs into the original's tick");
```

`OK` is `Ui_OkButton(0x1AC, 0x1B4, 0)` — the **corner picture** at (428, 436). The tick is
somewhere else entirely, and nothing in the module knew it existed. So the check was
accurate about the rectangle it named, the name was wrong, and it passed. That is the
*"passes for an accidental reason"* family in `docs/agents.md` with a new member, and the
sharpest one yet, because unlike the build-stamp test it was written *specifically* to
prevent this and still did not.

The replacement names no rectangle. `crates/l2-game/tests/right_column.rs` and
`divide.rs`'s `no_two_hotspots_on_this_screen_overlap` enumerate every box the screen tests
and compare them pairwise, and the geometry is asserted against the **player's own
`Lords2.exe`** rather than against our reading of a painter. A check that has to name a
thing can name the wrong thing; a check that quantifies over everything cannot.

**C93 — every control on screen `0x11` was 88 pixels from where the game
tests it, and the whole screen's input was in a function nobody had read.**

`Screen_FrameInput`'s `0x11` arm is **three exits and no verb**: the turn-ended latch, a
right release, and `Ui_OkButtonClicked`, all three writing `g_screenId = 0x04`. Every button
on the army-division screen is `Screen_HandleInput`'s

```c
else if (g_screenId == '\x11') Widget_Test(0, 0, &DAT_004DD388, DAT_0055321C);
```

— which is the **second of the three places input hides** that `docs/arms.json`'s `_note`
names, and which nothing had decoded. So `divide.rs` derived its hit boxes from the painter
above it, `Screen_SplitArmyRows` (`0x00419354`), whose two `Pl8_DrawFrame(g_miscCtySheet,
t + 0x2F, 0xA8 | 0x158, …)` calls it read as *"the parent's button"* and *"the daughter's
button"*.

They are the troop-type **pictures**. The buttons are at x **256 and 288**, in the 32-pixel
gutter between the parent's number and the daughter's icon, and had nothing over them. The
`y` was right and always had been, which is what made it look correct: `row_y(row) - 8` is
the table's `row * 0x20 + 0x78` exactly, so eight rows lined up perfectly at the wrong `x`.

This is the **fifth** hit-box defect on this project and the first where the *y* agreeing is
what hid the *x* disagreeing. The general form has not changed since the 58 × 47 sprite on
the 58 × 30 tile: **a painter says where a picture goes and a table says where a click
lands, and on this game they are routinely different places.** `map.rs`'s header carries the
standing warning; the new sentence to add to it is that a *partial* agreement between the
two is more dangerous than none, because it reads as confirmation.

`node tools/oracle/widgets.js widgets 4dd388 18` is eighteen lines and would have said so at
any point in the last month.

**C94 — `FUN_00437107` is a cut control strip for move-order mode, and settling
it needed a scan of the image rather than of the corpus.**

It had been left unfiled on purpose: `xref.js` found no caller, and *"xref found no caller"*
is not the exhaustive check `docs/arms.json`'s `dead` status requires. It looked like a
second unit panel — a `Hotspot_Test` over a two-record table for the selected army, behind
the same *kind 1, owned by the local player* guards `FUN_00437002` uses.

It is not. The tell is one global: `FUN_00437002` tests `g_pickedTileUnit`, *"the unit index
on the tile `Map_ResolvePick` just resolved"*, and `FUN_00437107` tests **`g_selectedUnit`**,
*"the unit a move order is being given to"* — which is screen `0x10`'s state and nothing
else's. Decoding its table settles it: `0x004DC5F0` is two 32 × 32 boxes at (104, 408) and
(184, 408), whose handlers are `if (g_mapZoom != 2) { g_screenId = 4; FUN_0041B032(); }` —
*open the information panel* — and `g_screenId = 0` — *cancel*. `Rect_Contains(0x10, 400,
0xF0, 0x40)` is the 240 × 64 frame around both, returning 1 so the strip swallows clicks
that miss its buttons. **An on-screen strip with an INFO button and a CANCEL button, shown
while an army is picked**, which shipped as right-click-to-cancel instead.

**How it was settled, because the method is the transferable part.** `xref.js` reads the
decompiled corpus, so it can only see calls the decompiler recovered; `widgets.js ref` scans
the image for the address as a dword, which is how a handler reaches a table. Neither alone
is exhaustive. Three scans over the **whole 1.3 MB file** are:

* every `E8` whose `rel32` resolves to `0x00437107` — **zero**;
* every `E9` likewise — **zero**;
* every dword anywhere equal to `0x00437107` — **zero**.

Those three cover the ways x86 reaches a function: a direct relative call or jump, or an
absolute address stored somewhere. A byte scan **over-approximates** — it reports any offset
whose bytes would decode that way, whether or not it is an instruction boundary — and that
is the right direction, because it makes *zero* a proof rather than a hint.

And it was **controlled before it was believed**, which is the half that is usually skipped:
run against five functions known to be reached, it finds `FUN_00437002` at `0x430586`,
`FUN_00438A91` at `0x430574`, `FUN_0043B412` at `0x430D03`, `FUN_0043B4CB` at `0x4308E7` —
and `FUN_004374C4` with no call at all and **one dword, in the garrisoned hotspot table at
`0x4DC5B0`**, which is exactly the case a call scan alone would have called dead. A tool
whose first run returns "nothing found" is a tool nobody has seen working;
`docs/agents.md`'s *"read the first three findings of every new tool's first run"* has a
mirror image, and this is it. `tools/oracle/reaches.js`.

The cluster is dead as a whole: the table is named in one place in the corpus, inside this
function, and each of its two handlers is pointed at exactly once, from that table.

**C95 — `docs/arms.json` counts arms and had four filed twice, and one
of the pairs disagreed about whether we had built it.**

Two agents enumerated screens `0x04`, `0x17` and `0x18` from two directions — one from
`Screen_FrameInput`'s ladder, one from the widget and hotspot tables — and both filed
records. The ids differ, so **set equality against the markers is perfectly happy**: the
check the file is built around cannot see this at all.

The pair that shows what it costs:

| id | group | status |
|---|---|---|
| `0x0043B412/supplies-county-picker` | management-screens | `missing` |
| `0x0043B412/supplies-pick` | panels | `reproduced` |

Same address, same screen, same call, opposite verdicts — and the `missing` one was on the
worklist for this branch as an arm to build, which is how it was found. The other three
pairs were `unit-panel-buttons` against `info-move`/`info-disband`,
`tile-panel-widgets` against `info-garrison-widget`, and `info-panel-edge-scroll-leaves`
against `info-edge-scroll-closes`.

All four are folded, each survivor carrying a `merged` field with the other's text so the
second reading is not lost. The `_note` now says to grep the file for an `addr` before
adding a record. **A duplicate cannot be checked mechanically here**, because the file's
whole design is that one function holds many arms — `Screen_FrameInput` alone is 49 of them
— so *"two records share an address"* is the normal case and not the defect. What would
catch it is the thing that caught this one: somebody trying to build an arm and finding it
already built.

The counts moved 185 → 193 records: −4 folded, +12 new, and **13 records that said
`missing` now say `reproduced`, of which one was already implemented and only lacked a
marker.** That last is worth its own sentence, and it is the next entry.

**C96 — the raise-army screen's right release was reproduced, recorded
as missing, and described as doing something it does not do.**

`docs/arms.json` `0x0042FF10/levy-right-commits` read: *"a right release … sets
`g_screenId = 0x0A` — the armoury — and runs `FUN_004AA90A(g_selectedCounty, g_levyMen)`,
which **COMMITS the levy** … So the two ways out of the raise-army screen do OPPOSITE
things, and the one a player reaches by habit is the one that acts."*

Two errors, and they are different in kind.

**The function does not commit anything.** `FUN_004AA90A` has since been named `Levy_Seed`,
and its body zeroes the eight basket slots, fills their available counts from the realm's
weapon stocks and puts the levied headcount into slots 0 and 7. Its own comment names its
three callers as *"every door into the armoury"* — `Sidebar_Button`, `RaiseArmy_Continue`
and this arm. It **prepares the armoury** for the number the slider chose. No man is levied
and no gold spent until *Create*, which is a button on the armoury and is
`Army_RaiseConfirm`. The record was written from the call site and the name it had at the
time, and `FUN_004AA90A` supports any story you like.

**And we already did it.** `RaiseArmyScreen::handle`'s right-click arm calls
`Game::seed_levy_basket` and replaces itself with the armoury — the arm, its side effect and
its destination, all three. It had no `// arm:` marker, so it counted as a miss, in the file
whose purpose is counting misses.

That is the failure mode of the marker rule stated plainly: **set equality catches a record
with no code and code with no record, and is silent about a record whose status is simply
wrong.** Eleven of this branch's thirteen conversions were real work; one was a marker; one
was a duplicate. A `missing` record is a claim about our own tree and nothing checks it —
the cheapest available remedy is that anybody picking an arm off the worklist reads our
source for it first, which takes a minute and would have saved this one an afternoon.

**C97 — the sidebar strip is 29 pixels tall and we had 30, because a plate's
height is not a hotspot's.**

`SidebarButton::rect` computed its height as `PANEL_END_TURN_Y − PANEL_STATUS_Y` — the
distance between two `Misc_cty` plates, which is 30. `g_sidebarButtons` records 0…4 are
`(x, 0) … (x, 29)` at the table's `0x1AE` offset, and `Hotspot_Test` is half-open on **both**
axes:

```c
if (mx < x0 + ox || x1 + ox <= mx || my < y0 + oy || y1 + oy <= my) /* miss */
```

So the strip is y 430 … 458 and **y 459 belongs to nothing** — the same one-pixel gutter the
table leaves between each pair of icons, once, horizontally across the whole strip. End Turn
is record 5, `(0, 30) … (161, 49)`: **161 × 19**, not the plate's 162 × 20, so its last
column and its last row are dead too.

Nobody would ever notice, and that is the point of writing it down: it was found by a check
that reads the table out of the player's own `Lords2.exe` and compares it with our constants,
and the module's *own* unit test had asserted the opposite — `assert_eq!(r.y + r.h,
END_TURN_BUTTON.y, "does not meet the end-turn strip")`. Two of our constants agreeing is
what that test measured. `docs/agents.md`'s rule about two artefacts maintained by the same
person in the same commit, in a file that had already been corrected twice for exactly this
kind of thing.

**C98 — the arm was reproduced on four screens and absent on the
fifth, which is worse than absent on all five.**

`Screen_FrameInput`'s epilogue runs `Minimap_Click` on **every** screen id but `0x12` and
drops the management surface on a hit. It is recorded per screen — the court, the job popup,
send supplies and the ratings each have a record — and the information panel `0x04` had
none, so it swallowed the press. A control that works from everywhere else did not work from
there, which is precisely the shape a player reports as *"sometimes the minimap doesn't
work"* and nobody can reproduce.

The enumeration that produced the per-screen records was of *shells*, and `0x04` graduated
out of that table before it ran. So the miss is not an oversight in reading the binary; it
is an arm that was **lost at a graduation**, which is the same failure `docs/agents.md`
records for the shell wrapper that used to do this generically for all seven. There is now a
test over every overlay we can build rather than a record per screen —
`no_overlay_swallows_the_campaign_minimap` — because a list of screens is a thing that goes
stale and a quantifier is not.

**C99 — The castle's surface model was inverted, and a besieger could not win because of it.**

`crates/l2-sim/src/siege.rs` carried `SURFACE_RAMPART = 5` and `SURFACE_BREACH = 4`, described as
*"the rampart"* and *"what a breached rampart patch becomes, and what `Siege_FindCellSurface4`
hunts for"*. An exhaustive search for **writers** of cell byte `+7` — 36 of them in the corpus —
settles all of it, and the second half of that sentence is false.

| surface | written by | what it is |
|---:|---|---|
| 1 | classifier `FUN_0047E7B2`, flooded from the two map corners | the open field |
| 2 | `Battlefield_BuildCastle`, terrain byte `0xEE` | the moat |
| 3 | classifier `FUN_0047E668`; `Siege_LowerDrawbridge`'s patch | ground-level ground outside |
| **4** | classifier `FUN_0047E52D` — raised ground beside a 5 | **the rampart walk** |
| **5** | classifier `FUN_0047E387`, flooded from the keep door and the bridge — **and `Wall_Smash`** | the bailey, and what an opened wall joins |
| 6 | build code 6, classifier `FUN_0047E263` | the keep and its `0x08` door |
| 7 | build code 7 | the bridge |
| **8** | build code 8, with flag `0x20` | **an intact, breakable wall** |
| **9** | `Wall_Collapse` (`FUN_0047DFE0`) | a wall cell a catapult brought down |
| 0x0B | build code 9, with flag `0x40` | the raised drawbridge |

Three of those are decisive on their own. `BattleMan_StateAttackWall` keeps swinging while
`Cell_NeighbourHasSurface(x, y-1, off, 8)` still finds wall to the north — so **8** is the standing
wall. `Wall_Collapse` writes surface **9**, not 4. And **nothing anywhere writes surface 4 outside
the classifier**, so *"what a breach leaves behind"* cannot be 4.

What survives from the old reading is the one place it was load-bearing and right: the accumulator
test really is `standing_on == 5`. But 5 is not the rampart — it is *the inside*, and a smashed
wall joins it. Read that way the two thresholds stop looking arbitrary: **a besieger outside chews
the gate at 20,000, and one who is already through chews the next wall at 5,000**, which is what
makes a breach spread.

Two of our own numbers followed the correction. `our_castle`'s wall stood at **elevation 2** where
the builder's structure code 8 writes **1** — and the height rule allows exactly one, so an opened
wall two cells high is a hole nobody can walk through. And the rampart walk sat directly against
the wall's inner face, where `Wall_Collapse` bills only neighbours at surface 5 — so a catapult
could bring the whole curtain down for a breach score of **zero**. Both were ours; both are
corrected; ablating either turns the new siege tests red.

**C100 — A breach is nine cells wide, and ours was one.**

`FUN_0049694F` is called from three places — both wall-attack states at the 5,000 rampart threshold
and again at the 20,000 gate threshold — always as `FUN_0049694F(mapX, mapY, 4)`, centred on the
**attacker**. It sweeps the 9 × 9 square that radius describes and, for **every** cell in it
carrying flag `0x20` or `0x40`, clears the flag and writes surface 5. It does not touch the
elevation; that is `Wall_Collapse`'s job and a different routine.

Ours opened the single cell the attacker had walked into. A one-cell gap in a castle wall is a
funnel, and the whole of the reported *"848 men outside, two garrison figures alive, 400,000
frames"* is consistent with it. The name is now `siege::smash_walls`, with `SMASH_RADIUS = 4`
beside it, and `siege::collapse_wall` is the catapult's separate routine.

A detail worth keeping because it is not the kind of thing anyone would invent: the wall's graphic
bump lands **one row south** of the smashed cell (`frame[+0x280] += 0x10`) while the drawbridge's
lands on the cell itself (`frame += 0x28`).

**C101 — A fresh siege opens at approach score 500, and the moat is what puts it back to zero.**

`docs/battle.md` §16.1 said `Siege_RestoreCastleDamage` *"overwrites the fresh
`g_siegeApproachScore = 500` that `Battle_Start` wrote a moment earlier"*. All three halves of that
are wrong. The 500 is written by **`Battlefield_BuildCastle`**, not `Battle_Start`; the restore
fires **only when `castleDegraded == 2`**, that is on a *repeat* assault, and otherwise zeroes the
county's fields and leaves the globals alone; and what actually overwrites it is the **moat** —
the raster's second pass hands terrain byte `0xEE` to `FUN_0047DCCE`, whose first statement is
`g_siegeApproachScore = 0`.

So there are two opening positions, and read against `Order_ToBreachOrStaging`'s three arms — under
16 hunt the ditch, 16 to 400 fall back on staging, over 400 **do nothing at all** — they are one
design:

* **a moated castle opens at 0**, the besieger's whole approach ladder is spent shovelling, and
  `Moat_Fill`'s one-to-four points a cell is what carries it past the `approach_score < 3` gate
  that four attacker handlers open with;
* **a dry castle opens at 500**, the approach is already done, and the only thing holding the
  ladder shut is `breach_score == 0` — which the siege engines are there to answer.

We started every siege at 0. On a dry castle that is a besieger who never leaves the approach
ladder however long the battle runs.

**C102 — `_DAT_0055307C` counts gaps in the wall; it is not the moat flag.**

`crates/l2-sim`'s `AiField` carried it as `moat_flag`, `[I]`, on the strength of nothing. Six sites
in the binary and they agree:

* **initialised** by `Battlefield_BuildCastle` to `(castleLevel == 0 || castleLevel == 3) ? 1 : 0`,
  or on the skirmish path from a twenty-entry table at `0x004D4A98` holding
  `1 0 0 0 0 1 0 0 1 0 1 0 0 0 0 0 0 1 1 1` — one entry per castle raster, seven of twenty set;
* **incremented** by `BattleMan_StateAttackWall` and `BattleMan_StateRamGate`, in both cases at the
  5,000-hit rampart threshold, beside `Wall_Smash` and the counter reset. `Wall_Collapse` does
  **not** touch it, and ours did;
* **read** three times: `BattleMan_StateRamGate` sends a ram away at `1 < it`, and
  `Order_ToCastleObjective` and `UnitOrder_SiegeDefMissile` branch on `it < 1`.

The seed rules the old name out on its own: the moat appears from castle level 2 upward and this is
set at levels **0 and 3**. `[V]` on the writers and readers; `[I]` on reading it as *"a gap"*
rather than some other per-layout property that a rampart breach also creates. It lives on
`SiegeState` — the county carries it between assaults at `+0x1F0` — and is mirrored onto the AI
every frame.

**And we already knew.** Filing this name tripped `symbols_md.js`'s promotion check, because
`docs/hypotheses.json` had carried `0x0055307C` since the first battle pass as
**`g_rampartCellsBreached`** — *"how many rampart cells this siege has knocked through"* — with an
honest caveat saying the mechanism was read and the purpose was a guess. That is the right answer,
near enough, written down weeks before `crates/l2-sim` called the same address `moat_flag`.

Nothing could have brought them together. The hypothesis register is keyed by **address**; the
field on `AiField` is keyed by **name and offset**, and no check in the tree relates a struct
field to the global it mirrors. So the project simultaneously held a good reading and a bad one
about the same four bytes, in two files that are both maintained, both checked, and never checked
*against each other* — and the bad one was the one the code ran on. It surfaced only because
promotion happens to be a move rather than a copy, which is a rule written for an entirely
different reason.

> **A contradiction between two documents is invisible until someone needs both.** The register of
> guesses and the code that acts on them are exactly such a pair, and the thing that finally
> connected them was a check about bookkeeping. `docs/agents.md` carries this one.

**C103 — `Path_LineIsClear` is not a line, not a predicate, and not free of side effects — and that is where 848 men were stuck.**

`docs/battle.md` §8.3 describes it as an early out: *"if the target is adjacent or
`Path_LineIsClear` succeeds, no search happens at all"*. True, and it hides three properties that
together are the difference between an army that presses an assault and one that stands in a field:

1. it seeds `g_pathCost` from the blocked template **and calls `Path_BuildBlockedMap`**, which
   writes 998 under every *friendly* figure — so a comrade in the way is an obstacle here exactly
   as terrain is;
2. it is **two greedy walkers**, not a Bresenham line. Both set out from the start; each step takes
   the eight-way direction toward the target and, when that cell is taken, rotates — one clockwise,
   the other anticlockwise, up to eight tries — so the walk *slips around* whatever is in the way.
   Eighty rounds of the pair, then it gives up;
3. and **it leaves its cost field behind**. When `Path_Search` skips the flood fill because this
   succeeded, `BattleMan_Step` runs `Path_Extract` on that field anyway and the figure comes away
   with the walked route, detours and all.

Ours was a strict Bresenham returning a bare `bool`, ignoring friendly figures, and
`pathfind::search` answered `NoSearchNeeded` **with an empty cost field**. So a figure whose next
step was taken by a comrade asked for a path, was told none was needed, and got nothing — and then
tried the same blocked step again, every frame, for ever, with `barred` at 0. One figure does that
invisibly. An army packed in front of a breach does it as a permanent deadlock: measured at **45
besiegers frozen in a block eight cells wide for 200,000 frames**, every one of them `Walking`,
every path empty.

`Grid::walk_line` is now that walk. It is applied at exactly one site — the `NoSearchNeeded` arm of
`BattleRunner::request_path`, which is only ever reached because a step was refused — and
deliberately **not** wired into `pathfind::search` as a general replacement for the line test.
Doing that was tried first and it is the more faithful change; it also moves every field battle,
and `battle-after.sav`'s fought verdict flips. The narrow form keeps that fixture exact and fixes
the deadlock, and the difference between the two is recorded here rather than lost.

**C104 — The mover never enforced the height rule, so the one constant the wall depends on could not be wrong.**

`docs/battle.md` §7 marks it `[V]`: *"a step is only allowed when the two cells' elevations differ
by at most 1, unless the destination's elevation is exactly 5."* `movement::can_step_elevation` has
said so since it was written and **nothing called it** — the pathfinder enforced the rule with its
own copy and `BattleRunner::enter` did not, so a figure walking straight at its target, which is
what a figure does whenever no search runs, climbed cliffs.

It is inert on a `.skr` field, where `Battlefield_BuildFromSkr` never writes byte `+4` and every
cell is at elevation 0, which is why no field-battle test could see it. It is not inert on a
castle, and it is the reason `WALL_ELEVATION` is load-bearing: with the mover enforcing height, a
wall built one cell too high is a wall nobody can walk through after it is opened, and the siege
tests go red. Without it, the constant is decoration.

**C105 — The garrison's twenty defence posts are the holes a catapult has made, and we invented twenty of them at deployment.**

`FUN_0048EE46` appends a cell to the twenty-entry table at `DAT_00553C80` that
`Siege_ClaimDefencePost` reserves from. It is the table's **only** appender in the whole binary,
and its only caller is `Wall_Collapse` — once per orthogonal neighbour left hanging by a catapult
shot. So a castle nobody has bombarded has **no defence posts at all**, `Siege_ClaimDefencePost`
returns 0 for every unit, and the garrison's handlers take their `cellOffset == 0` arm — the wall
slots — for the whole of that siege. `our_castle_ai_field` spread twenty posts along the gate wall
at deployment; it now leaves them empty and the collapse routine fills them.

**C106 — "The AI never orders siege engines" was a true statement about the wrong function.**

The hand-off that opened this branch reported that `siege::order_engine` has three callers and all
three are the player's, and concluded that *"an AI besieging a level-3 castle can never assault at
all."* The premise is exact and the conclusion is false. `order_engine` is the siege screen's `+`
and `−` buttons (`0x0043B681` / `0x0043B741`) — the original's AI does not press those either. The
AI's path is four calls above it:

```
Unit_ReachCastleBuilding  0x004686A0   an army walks onto a castle tile
  Army_BeginSiege         0x004A7CA2   four guards, no else
    Siege_Link            0x004A7E0A   the two back-pointers
      Siege_Prepare       0x004A7EB5   clear the records, and for an AI, order
```

All four are implemented and `l2_kingdom::siege::prepare` is faithful, doctrine byte included. What
was missing was **anything that travelled the road**: every siege test in the workspace staged its
besieger by hand and therefore had to order the engines by hand too, and one of them wrote the
conclusion above into its module header, from where it was briefed onward as a fact about the
engine. `crates/l2-kingdom/tests/ai_siege.rs` now walks an AI army onto a castle and asserts the
order that comes out — 4 towers, or 2 towers and a ram, or 3 catapults and 2 towers, by the lord's
`+0xA0` — and the l2-game header is corrected.

It is the same shape as C71: **a true statement about one branch, promoted to a statement about the
subsystem, in prose, between agents, where no check in the tree can reach it.** The defence is the
same and it is one word — name the function. *"No AI calls `order_engine`"* is the same finding and
cannot be promoted by accident.


**C107 — A comment promised "until the real font is decoded", the font was
decoded, and whole screens went on being written in our debug font for months.**

The draw-call audit (`docs/draws.md` §8) set out to count what each screen draws and found
something the count cannot see: **a screen can reproduce every draw call the original makes
and still be entirely placeholder.** Six screen modules — `castle.rs`, `diplomacy.rs`,
`siege.rs`, `job.rs`, `menu.rs`, `menubar.rs` — put **nothing at all** on the canvas through
the game's own fonts or artwork. Every mark on them is `l2_view::text`'s hand-authored 5 × 7
bitmap font and `widget::panel`/`frame`/`button`'s rectangles. `castle.rs` also draws nine
English captions written in our source — *"SELECT A CASTLE TO BUILD"*, *"1 SEASON TO
BUILD."*, *"BOOSTS TAX REVENUES BY %"* — where `Screen_CastleBuild` fetches `L2.eng` group
71, and `menu.rs` draws the game's own title as *"LORDS OF THE REALM II"* when `L2.eng`
group 11 index 0 says *"Lords of the Realm 2"* — a string `crates/l2-game/tests/shell.rs`
has asserted for weeks.

**Nobody did anything wrong.** `crates/l2-view/src/text.rs`'s header said, in as many words:

> *"the original's glyphs live in `Font_c2.pl8` … that file is an open question … so the
> interface draws its own letters **until the real font is decoded**."*

By then `crates/l2-game/src/shell/font.rs` had decoded it: `Fntl2_14.pl8` and
`Fntl2_22.pl8`, through the 128-byte character-to-frame table at `0x004D71D0` that
`Glyph_Draw` (`0x00402A14`) indexes, with a mapping that checks itself on descenders. And
`Font_c2.pl8` was never the file the game draws from — `docs/audit.md` records that
`font_c2` does not appear among `Lords2.exe`'s strings at all and shares 103 of its 108
frame records with `Fntl2_9.pl8`.

**The chain is fully traceable and every link is a document.** `docs/audit.md` F21 had
already found this project's own *Open questions* list stale in **four of its six bullets**,
one of them the `Font_c2` RLE puzzle, marked *resolved* by `docs/formats/pl8-failures.md`
§5. Nobody removed the bullet. `text.rs`'s header cited it. Screens written afterwards read
that header and reached for the 5 × 7 font. An audit found the staleness, a comment
inherited it, and a third of the interface was drawn in the wrong font behind a green suite.

> **A document that promises *"until X"* keeps promising it long after X.** Nothing goes red
> when the named condition is met, because the condition is in prose. This is the sibling of
> *prefer a shape that cannot be wrong to a check that notices when it is*: the remedy is
> not to write better comments but to **make the consequence a number somebody prints**.
> `tools/draws/screendraws.js` prints the split per module and
> `tools/figures/figures.js` keeps it out of anyone's typing, so the day the real font
> landed the number would have moved on its own.

The stale bullet is struck from *Open questions* below in the same change, because leaving
it is the whole defect.

**C108 — We draw the word "OK" where the original draws a picture of an arrow going
into a hole.**

`Ui_OkButton` (`0x0040D1BC`) sets `DAT_005C9288 = 0x33` and blits `System.pl8` frame `0x33`
— decoded, and measured rather than described: of that sheet's 84 frames it is among the
darkest, 15.3 % near-black against a median frame's 1.2 %, because it is a cursor arrow
pointing into a small black hole. **It is not a tick, and it contains no letters at all.**
A dozen of our screens draw the literal characters `OK` in that corner, and several draw
`X`, `YES`, `NO`, `CLOSE`, `MAX`, `ALL`, `AUTO`, `SPLIT`, `DISBAND` and `CANCEL` on buttons
whose originals are `Widget_Draw` frames — the tick at 29, the cross at 31, plus at 68,
minus at 66.

This is a third kind of invention and it needed its own name, because the two we had do not
fit: it is not a *wrong string* (there is no string) and not a *missing draw* (something is
drawn). It is **text in a place that has none**, and it is invisible to every check that
compares what a screen shows against what a screen should show, because both agree there is
a button there.

`tools/draws/screens.json`'s `literals_ours` is where each of these now carries a verdict,
and `crates/l2-game/tests/draws.rs` asserts set equality between that list and the source in
both directions, so the category cannot grow quietly — `docs/agents.md`'s rule for naming a
thing that should not happen.

**And a quirk found in the same reading, already sitting in the symbol comment and acted on
nowhere.** `Ui_OkButton` stashes **only the last call's** position into `DAT_0055CE78` /
`DAT_0057C8A0`, which `FUN_0040E7E4` hit-tests on a left release. So **a screen that draws
two OK buttons makes only the second one clickable.** `Screen_BattleOutcome` draws three,
`Screen_DiploDialog` three, `Armoury_LoadScreen` and `Panel_JobDetail` two each. Whether
those are branches (one per frame, and the quirk is harmless) or successive calls in one
pass (and two corners are dead) is a per-screen question the audit records rather than
assumes.

**C109 — Two buttons on the merchant's stall, with their count zeroed
and their handler gutted to a `return`. The cut sheep row, in a second place and a shorter
story.**

The pilot found that `g_sendSuppliesWidgets` holds **eight** records and every caller passes
**six**, so two buttons exist in the data and are never drawn — findable only by reading the
table rather than the call. The audit was told to look for another and found one, and it is
the same shape reached from the other end.

`Screen_DrawWidgets`' `0x08` arm is
`Widget_Draw(0, 0, &DAT_004DD808, DAT_00569500)`. Three facts close it, each independently
readable:

* **`DAT_004DD808` is exactly two 24-byte records**, bounded by `DAT_004DD838` where the
  trade screen's table begins — two buttons at (256, 452) and (288, 452), frames 27 and 25,
  hotspot ids 0 and 1, on the stall's bottom bar left of the corner picture.
* **`DAT_00569500` is written `0` by `Screen_Merchant` and by nothing else in the image.**
  Its only other appearances are the two reads above — the draw and the hit test.
* **`FUN_0043527B`, the handler both records point at, is eleven bytes:**
  `void f(void) { return; }`. Nothing points at it but those two `.data` slots.

So the count is zero, the picture is never drawn, the hit test never fires, and the handler
does nothing if it did. **We draw neither, which is what the original does** — this is
recorded as a *finding about the game*, not a defect of ours, and it is here rather than in
`docs/bugs.md` for that reason.

Worth setting beside the supplies row, because the pair says something the single instance
does not: **cut content leaves different fossils depending on when it was cut.** There the
records survive at full length and the *count* was shortened; here the count was zeroed and
the *handler* was replaced with a stub. An audit that only checked "is the table longer than
the count?" would have found the first and walked past the second.

**C110 — Seven instances of one confusion in an afternoon, and it is the shape of the
API rather than anybody's carelessness.**

`Pen::body` returns `x + advance + TRAILING` — an **absolute** x, so that a caller can chain
runs. The original's equivalent is `g_penAdvance`, which is the **width the label
advanced** — a relative number — and every painter writes
`Eng_DrawString(g, i, g_penAdvance + 0x28, …)` with the label's own x added back.

So a faithful transcription of a painter produces `pen.body(canvas, X + w, …)`, and that is
wrong by exactly `X`. The draw-call audit found it seven times in one afternoon, by four
different agents, in four files none of them shared:

* **`Pen::count` itself**, which drew its noun `x` pixels right of its number on every
  screen that used it. Measured: `number(x = 100, 5)` returns 116 and `count(x = 100, 5)`
  put the noun at **216**.
* **`court.rs`** — the lord's name **80 pixels** right of where the game puts it.
* **`ratings.rs`** — *"Scored"* and the score walked off the block.
* **`info.rs`**, four times: the army's home county, its year formed, its wages and its
  moves left.

**Not one of these had a test, and none could have had a useful one**, because each draws a
label and a value and nothing after them: there is no second thing for the misplaced text to
collide with, and a canvas diff has nothing to compare against. It is
`docs/agents.md`'s *a test that drives the picture from the wrong field passes for ever*
with no test at all — the defect is visible only to somebody holding the original's
coordinate beside ours, which is precisely what a draw-call listing is.

**The fix applied is the seven call sites. The fix that would end it is a type.** `Pen`'s
methods should return a `PenX(i32)` that cannot be added to an `i32`, so `X + w` stops
compiling — `docs/agents.md`, *ask whether the mistake can be made unrepresentable before
asking what would notice it*. That was not done in this change because six agents held the
screen modules open at the time and a signature change across all of them was the wrong
thing to land mid-flight. It is the right next move and it is small.

**C111 — `screens/menu.rs` is a whole screen of ours that the shipped binary cannot
reach, and it was inflating the audit's placeholder count.**

`crates/l2-game/src/main.rs` boots to `ScreenId::Setup(SetupPage::Title)` — the *real* front
end, which `setup.rs` reproduces with 41 draws through the game's own artwork and is the
healthiest module in the whole draw audit. Nothing outside `crates/l2-game/tests/machine.rs`
ever pushes `ScreenId::Menu`. So `menu.rs` — a two-item main menu of ours, drawn entirely in
the 5 × 7 font, carrying the misspelled title above — is an **invention that is also dead**.

That pairing is why the draw inventory carries `reachable` beside every record, and it is
the same hole `docs/agents.md` records for screen `0x28`: *counting arms cannot tell you
whether a screen is reachable*, and every arm audited on a screen nothing selects counted
toward a denominator it should not have been in. A draw audit has the identical hole and has
it in **both** directions — a painter in the binary that no `mov byte ptr [g_screenId],
imm8` can select, and a screen of ours no transition reaches.

`screens/index.rs` is the contrast worth keeping: it is also ours, also drawn in the 5 × 7
font, and it is **correct**, because it is titled as ours on its own face so that a
screenshot of it can never be mistaken for something the original drew. The difference
between an honest scaffold and an invention is whether it says which it is.

**C115 — Two features that are each right, and a seam between them that nobody
tested, because every test of it presses End Turn in the same breath as the order.**

A player: *"It was me attacking a town and it just immediately resolved."* He marched his
army onto an enemy county and the autocalc settled it — no *"Will you take the field?"*, no
battlefield, no report. Screen `0x12` had been working for days and every test of it was
green.

**The cause is an `and` nobody wrote down.** Two changes landed in the same week:

* **`turn::tick_units_only`** — `Units_Tick` on an ordinary frame, so an army the player has
  just ordered walks away while he watches instead of standing still until End Turn. Right,
  and it is the original's own shape: `Units_Tick` is called from the frame loop beside
  `Turn_Tick` and never reads `g_turnPhase` (C35).
* **A turn is paced over frames**, with `TurnStep::Running` and a `resume_turn` per frame.
  Also right.

Between them, the army now **arrives before End Turn is pressed**. `tick_units_only`'s battle
arm carried a note that read as a complete argument — *"this is not a turn, there is no
`TurnProgress` to suspend, and a screen `0x12` raised from an idle frame would have nothing
to carry on afterwards"* — and every clause of it was true. The conclusion was not.
`Battle_ChooseSettlement` (`0x004A6A30`) has no opinion about which frame an army arrived on:
`Unit_EnterOccupiedTile` and `Army_AttackCounty` call it from inside `Units_Tick`, and it
writes `g_screenId = 0x12` on the spot. What stops the campaign afterwards is `Units_Tick`'s
own latch abandoning the sweep, **not** a turn being in flight. There is no "idle frame" in
the original for a battle to be raised on; there is one frame loop, and the gate is in it.

So the whole of ours was the wrong half of the door: the player's battle met
`Game::field_policy`, which is `Answer::Decline`, which *is* the autocalc — and `record` then
dropped the report on the floor too, because there was no `TurnProgress` to hang it on. A
decline and a never-asked look identical from the campaign afterwards, which is exactly what
he saw.

**Why eight passing tests could not see it.** `military.rs`'s prompt fixture
(`a_battle_is_about_to_happen`) gives the march order and the very next event is `press 'e'`.
So does the siege one. So does every test in `siege_battle.rs`. The army is therefore *always*
inside the turn machine when it arrives, and the door that a player actually uses was never
opened by anything. `docs/agents.md`'s hand-staged-fixture entry is the same failure one step
out: there, every siege test staged its besieger instead of marching one; here, every prompt
test ends the turn instead of watching one. **The fixture was not wrong about the state — it
was wrong about the frame.**

That is the generalisable half, and it is not "write more tests":

> **When two features are landed a week apart and each is tested alone, the thing neither
> test covers is the *ordering* they made newly possible.** Ask what became reachable that
> was not reachable before, not what changed.

Nothing changed in the prompt, the gate, or the turn loop. What changed is that an army can
now arrive at an enemy on a frame where no turn exists — a state that was unrepresentable
the day the prompt was written, and therefore not something its tests could have omitted.

**The fix names the gate rather than the frame.** `tick_units_only` answers all three of
`Battle_ChooseSettlement`'s settlements exactly as a turn does — `Silently` autocalcs and
shows nothing, `Reported` autocalcs and shows `0x13`, `Prompt` raises `0x12` — and suspends
the campaign in a `TurnProgress` marked `idle`, which carries the question and the unseen
report and **may not enter the phase machine**. That last word is load-bearing: a player who
answers a battle he was watching must not have his season wound on behind him, and
`answering_a_watched_battle_settles_it_and_does_not_end_the_turn` is the assertion, ablated
by deleting the `idle` guard in `advance`.

**One thing the report of this was wrong about, recorded because the number is quoted.** The
brief pinned `battle-after.sav` at *"3,900 ticks, defender (182, 8)"*. It reproduces at
**3,100 ticks, defender (182, 22)** — on `main`, unchanged by any of this, verified by
running `tests/seam.rs` against `main`'s `turn.rs` in the same tree. The verdict, the
attacker's `(178, 0)` and the militia holding the field are all exactly as pinned; only the
two numbers that nothing asserts have drifted, which is what happens to a figure that lives
in prose. `tests/seam.rs` prints them and asserts the verdict, which is the right division.


**C116 — The music has never played, because the front end is pushed under the game rather than replaced by it.**

A player reported *"not hearing any sound."* The start-up line said `sound: on (771 wav files
found)`, so the device opened and the install was found. Nothing was wrong below `audio::scene`:

```rust
if let Some(ScreenId::Setup(_)) = machine.ids().first() {
    return Scene::FrontEnd;
}
```

`SetupScreen`'s Start button returns `Transition::Push(ScreenId::Campaign)`, not `Replace`. So the
title screen stays at the **bottom** of the stack for the whole session, `scene` answers `FrontEnd`
in every state a running game can be in, and `follow` calls `stop_music` sixty times a second.
**No music has played since the audio layer landed**, on any screen, in any game.

The predicate meant *"is the front end still up?"*. What it implemented was *"was the front end
ever up?"*. Those two agree on exactly one kind of machine — one built by hand with a single screen
on it — and that is what the test was:

```rust
// Through the real entry point, on a real machine, from the real save.
let machine = Machine::new(ScreenId::Campaign);
```

The comment is the finding. Every word of it is false about the *stack*, and the assertion it
guards is correct about everything else — the save loads, the share is 7 %, the ladder answers
`Scroll1`. It is `docs/agents.md`'s *a test that drives the picture from the wrong field*, with a
stack instead of a field, and it is C27's tenth instance: a rule with no way in, behind a green
suite.

**Three things this changes about how the audio layer is checked**, and the third is the one worth
carrying:

1. `scene` now asks the **whole** stack, in the original's own three phases: anything is
   `ScreenId::Battlefield` → `Music_StartBattle` (`0x00477B2F`), which `Battle_Start`
   (`0x004778A0`) opens with; every screen is a front-end screen → silence, because nothing before
   `Game_NewGame` reaches either picker; otherwise → `Music_StartCampaign` (`0x00499ACA`), which
   `Game_NewGame` ends with. `[V]` from the decompilation, read at the time of writing.
2. `before_the_campaign` is **exhaustive over `ScreenId` with no `_` arm**, so adding a screen does
   not compile until somebody has said whether music plays behind it. That is the only defence that
   would have caught this, because every check that *read* `scene` agreed with it. Verified by
   adding a variant: the compiler names `audio/mod.rs` first.
3. **`App::listen` moved out of the binary into `audio::Director`.** It had been six lines in
   `main.rs`, and an integration test cannot link a binary — so the only test available was one
   that *re-typed* those six lines beside its own assertions. That test passes with `main.rs`
   deleted. `docs/agents.md` warns about two artefacts maintained by the same person at the same
   time; re-typing a function into its own test is the purest form of it, and it is invisible
   because the copy is correct.

The link that still cannot be typechecked — that `main.rs` calls the director at all — is held by
a source-text check with a message that says so. Ablating the call turns exactly that test red and
leaves the other six green, which is the honest picture of what it is worth.

**And the count, which nobody had quoted — and which was wrong the first time it was.** Of the
install's 771 `.wav` files the engine could reach **0** before this and can reach **11** after it:
five campaign tracks, **four** battle tracks, `ff_msg.wav` and `ff_batl.wav`.

The draft of this paragraph said **12**, counting `battle1`…`battle5` out of the table. `Battle5`
ships, decodes, and is unreachable: the counter that selects it is `DAT_0057A0F0`, the third battle
mode `BattleKind` deliberately does not name because it is unidentified. Reading a table is not
driving it, which is the same lesson as everything above, arrived at from the other end — and it is
why `Audio` now records what it actually opened and
`audio_wiring.rs::eleven_of_the_installs_771_sounds_are_reachable` drives every scene the policy
can produce and asserts the *set*. A new call site moves the number by itself; a name that stops
being reachable goes red carrying its own name.

The loudest of what remains is `click3.wav` — `Widget_Test` (`0x0040DA1E`) plays it on every widget
press from one call site, because the original has one hit-tester and we have one per screen. It is
not an audio change.

**C117 — The campaign's *Start* button is a different button, and the world it was starting was the player's own autosave.**

A player reported it in one line: *"Original campaign no longer puts me in the first
starting map for some reason."*

**Where the map comes from.** The original campaign's first map is `L2_maps.dat` slot
**17, Quaintville** — four counties, two players. Three independent sources agree, which
is worth listing because each one alone would have been a claim:

1. `Campaign_LoadEntry` (`0x00499E5D`) reads column `+0x00` of row `g_campaignMap` of
   `g_campaignTableA` (`0x004D8E18`), and row 0 of that column is **17**. `L2.eng` group
   101 index 17 is *"Quaintville"* and `L2_maps.dat` slot 17 has four counties.
2. The shipped `Readme.txt` — the v1.03 errata, and `CLAUDE.md`'s first-class oracle —
   says it in words: *"Quaintville (pg128) … The new map is an 'N' shaped map for **2
   players only** and contains **4 counties**. **This is the first map of the campaign
   (Play Now!!).**"* Two players only matches `g_aiLordCount = 1` on that row exactly.
3. The install's `old_turn.sav`, written by `Lords2.exe` itself part-way through a
   campaign, carries `g_scenarioIndex = 17` and `g_countyCount = 4`.

**What we were doing instead.** `SetupScreen::start` built the world from the *map list's*
slot, and the campaign never touches the map list, so it started slot **0, England** —
the campaign's **fifth** map.

**Why the word "no longer" is the whole of the finding, and it is not the story the brief
had.** The brief supposed that before C62 every path gave England turn one and that the
campaign's first map coincided with it. It did not. Before C62, *Start* built no world at
all: it applied the settings to whatever `Game` already held, and what `Game` held was
`crate::scenario::load`'s read of the install's **`lastturn.sav`** at boot. The player had
spent the day playing the campaign in the original, so his `lastturn.sav` **was
Quaintville**. Our engine was not starting the campaign; it was resuming his own saved
game, and it happened to be exactly the right map. The startup line he quoted —
`4 counties, 1 owned by realm 1, Autumn 1269` — is `main.rs` printing that save, and the
year is the proof: no fresh start can be in 1269, because `Game_NewGame`'s single
`Season_Advance` lands in Winter 1268.

So this is `docs/environment.md`'s own warning arriving in a place nobody had put it:

> **`lastturn.sav` inside a game install is the rolling autosave.** … Nine tests treated
> one particular `lastturn.sav` as a fixed fixture, called it "the shipped save", and went
> red the first time somebody played for ten minutes.

Nine *tests* were fixed by naming fixtures. The **application** still boots from that
file, so the correct behaviour of a feature depended on what the person had last played in
a different program. That is a coincidence that cannot be tested for and does not survive
being noticed.

**The arm, and it is a branch rather than a button.** `FUN_00433155`'s hotspot-2 arm —
page 4's *Continue*:

```c
else if (g_uiHotspotId == 2 && (g_multiplayer == 0 || DAT_0057C940 != 0)) {
    if (DAT_0057D320 == 1) {        /* a campaign was chosen on page 5 */
        Campaign_LoadEntry();       /* 0x00499E5D — the row, not the map list */
        Setup_StartGame();          /* 0x004329EC */
    } else {                        /* a custom game or a skirmish */
        ...g_setupPage = 7 / 8 / 0xB / 0xC, picked by DAT_0055302C...
    }
}
```

Both limbs were wrong. The campaign limb did not exist, and the `else` limb **started a
game**, where the original walks on to the page that chooses one. `DAT_0057D320` is set by
`Setup_ChooseCampaign` (`0x00433461`) and cleared by `FUN_00432B05` and three arms of
`FUN_00432CC8`; it is now `SetupScreen::campaign`, and the counter and track it also
writes are `Campaign::new(track)`, which this project had already read out of that same
function months ago and never called.

**`Campaign_LoadEntry` is not `Setup_CommitOptions`.** The eight columns of a campaign row
*are* the committed globals, written straight over whatever the custom page last set, and
five more options are forced: `g_optTimeLimit = 0` and `g_optFightHumansOnly = 1` before
the row, `g_optAdvancedFarming`, `g_optExploration` and `g_optArmiesEat` zeroed after it.
`CampaignMap::settings` is that function. One column means something different on the two
paths — `g_startingGoldChosen` is a *value* in the table (5000) and an *index* on the
custom path — which is why this could not be written as "look the twelve up and commit
them".

**Why neither `tests/newgame.rs` nor `tests/setup.rs` caught it, which is the more useful
half.** Not because they passed for an accidental reason: because **there was nothing
there to pass**. Every test in both files begins `SetupScreen::new(SetupPage::Custom)` and
presses page 7's *Start*. **No test in the workspace had ever pressed page 4's
*Continue*.** `tests/text.rs` reaches page 4 and only types into it. The campaign side had
no coverage at all, and `crate::victory::Campaign::current()` — the accessor that turns
the counter into a map — had **no caller outside `victory.rs`'s own unit tests**. That is
`docs/agents.md`'s *"a field is only tested if something a test reads was written by
something the game runs"*, one level up: a whole *table*, correctly read out of the binary,
correctly unit-tested against itself, and wired to nothing.

The lesson generalises past this bug. A unit test of a data table proves the table was
transcribed; it says nothing about whether anything reads it. **When a table is added, the
thing to ask is not "is it right?" but "what calls it in a game?"** — and if the answer is
"the tests", that is the finding.

**Two documented claims falsified on the way.**

- `screens/setup.rs`'s `go` and `docs/arms.json`'s `0x00432B05/name-field-open` both said
  arriving at page 4 *from page 5 or 6* does not re-seed the name field, "because those
  arms set the page and nothing else". `Setup_ChooseCampaign`'s third statement is
  `Edit_Begin(&g_options, 0x10, 0xC0, 0)`. All four writers of `g_setupPage = 4` re-seed;
  the asymmetry does not exist. The code was already right — only the sentence about it was
  wrong, which is exactly why it survived: nothing behaved differently.
- `docs/symbols.json` calls `Setup_ChooseCampaign` *"Setup page 4"*. It is page **5**'s
  handler; page 4 is where it goes.

**Open, and not resolved by guessing.** `Readme.txt` line 147 says of the campaign: *"The
'custom' settings for each of these maps are preset, **except for the advanced settings of
Farming, Foraging and Exploration, which you can use or not at your pleasure.**"* Those
three are exactly the three `Campaign_LoadEntry` zeroes. Either they are changeable from
the in-game options screen after the map loads, or the errata is describing the intent
rather than the build. We reproduce the binary, which zeroes them; the sentence is recorded
here so the next person to open that screen can settle it.


**C118 — the test existed, was well named, passed, and the control was
broken the whole time.**

A player: *"Rations slider moves but is inoperable, no information about feeding peasants is
available."* Two sentences, one cause.

`crates/l2-game/tests/screens.rs` has had
`the_ration_split_slider_sets_the_field_the_original_sets` for weeks. It clicks the track,
asserts `ration_split == 37`, steps the caps, and passes. It is about the right screen, the
right gesture and the right field, and it is **useless**, because `Ration_SetSplit`
(`0x0043A5A9`) is not a setter and the whole defect was in the part after the write:

```c
rationSplit = split;
Ration_Apply(county, g_season);                    /* the food pass, on the spot */
...the search...
Labour_Allocate; County_RefreshEstimates;          /* twice */
Panel_Ration();                                    /* repaint */
```

Ours wrote the field and returned — and said so, in its own doc comment, under the heading
*"What this does not reproduce"*. So the thumb travelled and every number on the panel stayed
where it was, which to a player is indistinguishable from a control the game ignored.

**This is the thirteenth accidental pass this week and the first where the check was exactly
about the right field.** The others were proxies — a pixel count that measured the artwork, a
marker rule that agreed with the data by luck. This one names the field the gesture writes,
and the gesture *does* write it. The rule it breaks is subtler than *state the predicate you
mean*: it is that **the claim was the effect and we asserted the cause**, and for a control
the two are only the same thing if something downstream is listening.

The replacement asserts the effect and masks out the thumb: draw the panel, drag the slider,
draw again, and require pixels to differ **outside the slider's own rectangle**. The mask is
the point — a thumb that moves is what the player could already see.

**And the fixture could not demonstrate it, which is a rule rather than an inconvenience.**
England turn one, county 8: 435 people, 101 head. The standing herd feeds five people a head
*without being slaughtered*, so 505 mouths' worth of dairy covers 435 and the county eats
nothing — `herd_eaten` and `grain_eaten` are zero at **every** split. The slider there is inert
in the original too. So the test cuts the herd and says why; and `docs/rules.md` now has the
rule, because a player who moves that dial and sees nothing has found a fact about his county
and not a bug. Very possibly *this* player, on *this* county.

**C119 — a `[V]` symbol whose comment asserts the opposite of its body,
and twenty call sites inheriting it.**

`Ui_DrawNumberRight` (`0x004030C6`) is `Ui_NumberToBuffer` and then `FUN_004025D7`:

```c
FUN_004025D7(s, x, y, width, font, colour) {
    local_c = (width - TextWidth(s, font)) / 2;
    if (local_c < 0) local_c = 0;
    Ui_DrawText(s, local_c + x, y, font, colour);
}
```

That is **centring inside `width`**. `Ui_DrawCentred` (`0x00402C5E`) calls the same function
with the same arguments. The two are one alignment under two names, and only one of the names
is true.

`docs/symbols.json` carries it as `[V]` with the comment *"Ui_DrawNumber, right-aligned inside
width"* — so the error is not merely in the name, it is **inside the verification**. That is
the tenth instance of *a name is a claim* and the first where the claim was in the tier that
exists to stop claims. The lesson to carry: **`[V]` records that somebody read it, not that
somebody read it correctly**, and a wrong name with a wrong verified comment is the most
expensive object this project can produce — it is believed twice, once for the name and once
for the tier, and there is nothing left to contradict it.

It reached us the way it always does. `screens/county.rs` right-aligned the ration panel's
number columns because the symbol said *right*; the numbers sit in 64-pixel columns at
x `0xD0`, `0x10A` and `0x144` and belong centred in them. Two of the twenty call sites are on
that one panel. The rest are unaudited and the correction is the coordinator's to make in
`symbols.json`.

Found by reading the callee, prompted by a player who could not read a panel. Three
independent readings now agree; two of the three are on unmerged branches, and the claim that
`docs/draws.md` already carried it was checked and is false — the finding is real and its
stated location was not, which is worth recording because *checking the artefact rather than
taking the assertion* is the only reason this entry says what it says.

**C120 — the row that said `NOT SIMULATED` was three multiplications of fields
we already had.**

The ration panel's **Fed** row is three numbers, and `screens/county.rs` printed the words
*"NOT SIMULATED"* across it with a comment explaining that county `+0x16C`, `+0x170` and
`+0x174` are *"not in l2-kingdom at all, so there is nothing to put here"*.

They are not fields. They are products:

```c
county[+0x16C] = county.herd       * g_dairyPerHead;   /* fed by the standing herd */
county[+0x170] = county.grainEaten * g_foodPerSack;    /* fed by the grain eaten   */
county[+0x174] = county.herdEaten  * g_foodPerHead;    /* fed by the beasts killed */
```

Every input was already a `County` field and every constant already in `Tables` —
`ration::food_from_dairy` **is** the first line. They are derived here rather than stored,
because a fourth copy of a product is a fourth thing that can go stale.

The player's second sentence was *"no information about feeding peasants is available"*, and
that row is literally the information about feeding peasants. **The absence was recorded, in a
comment, next to the words on the screen — and read as a conclusion rather than as a
question.** That is the failure worth naming: an honest `NOT SIMULATED` is a better state than
a silent gap, and it is still a state nobody re-examined, because a comment saying *there is
nothing to put here* answers the question it also raises.

The third number is the one that matters, and it is why this entry is filed beside
C118 rather than as a cosmetic fix: *people fed by the standing herd* is
what tells a player his county eats nothing, which is what makes his slider inert. **The panel
had the answer to the complaint about the panel, and we were not drawing it.**

**C121 — dropping peasants on a switched-off industry turns it on, and
that is why our quarry looked full of idlers.**

A player: *"when I put peasants into a quarry they show as idle, which should be impossible."*

The drawing is faithful. `Village_RebuildIcons` (`0x0045161E`) draws every worker past a zero
useful ceiling in the **surplus** frame, and `g_peasantIcons[8]` — idle townsfolk — is the
*same frame*, out of the shipped table at `0x004D6808`. Surplus and idle are one picture, and
he was reading it correctly.

What we were missing is one line upstream. `Labour_Move` (`0x00439B52`) opens with
`FUN_00439CC2(county, from, to)`, which reads **only the destination**:

```c
if      (to == 6 && industry[0].hasResource) rec = 0;   /* wood  */
else if (to == 5 && industry[3].hasResource) rec = 3;   /* stone */
else if (to == 4 && industry[1].hasResource) rec = 1;   /* iron  */
else if (to == 7 && industry[2].hasResource) rec = 2;   /* smithy */
if (rec != 999) { county[+0x297 + rec*0x18] = 1; Industry_UpdateSiteTile(county, rec); }
if (to == 3) county.field_0x1B0 = 1;                    /* and the castle switch */
```

**The drop switches the industry on**, and `+0x1B0` — which `labour.rs` documented as *"a
switch the player throws by clicking the castle on the map"* — has a second writer nobody had
found. So the state we put the county in, off *and* staffed, is one the original cannot reach
by this route, and the surplus icons are the honest picture of a state that is ours.

**The evidence path is the entry.** The player's first account was *"it does let you drop
peasants off in a turned-off industry, IIRC"* — flagged as recollection. He then corrected
himself to *"it boots those people and reassigns them"*, which was **wrong in the other
direction**. Settled at the byte: the drop is accepted, nobody is booted, and the switch flips.
He then ran the experiment in his own copy — *"if I add people back in, it says it's
operational"* — and that agrees with the byte, from a direction the decompiler cannot reach.

Three things follow, and the third is the one to keep:

* **A memory that fits is not evidence.** This is the second time a remembered detail from him
  has reversed; the first was the tower and ram costs, where the arithmetic closed either way.
  He flagged both himself, which is why the process worked.
* **A correction is still a lead.** The corrected version was wronger than the original and was
  offered with more confidence, because a correction sounds like the end of a process.
* **A running copy of the game is a third kind of oracle and we barely use it.** We have the
  binary as a static oracle and `Readme.txt` as a documentary one; a person who can perform an
  experiment is the only one who can answer *what does it say on screen*. His observation cost
  him thirty seconds and settled what an hour of reading had left at `[D]`.
  `docs/oracle-requests.md` asks him for saves; it should ask him for **observations** too.

The tile panel's *"operational"* / *"not operational"* line is a read-out of this same byte and
is one of the ~150 draw calls that panel is missing. Handed to the map-draw agent rather than
built here.

**C112 — Three tables and two keys for one fact, and the player could see it because two of
them were on screen at the same time.**

A player, on a build with C63 in it: *"The sovereign land text has the wrong colours. When I
start, the counties seem to have the right colours — with Bishop being magenta, the Knight
being yellow, the Countess being blue, the Baron is black (at least, because I picked red) —
but the text doesn't match that."*

**The report's shape is what makes it strong.** He is not saying a colour looks wrong; he is
saying two things that name the same realm disagree, and one of them is right. That rules out
half the space before anything is read: the *data* reaching the screen is fine, because the
minimap is drawing it correctly, so the fault is in the second consumer.

**What the binary does.** `CountyStrip_Draw` passes `g_realms[owner].field_0x8` as the pen for
all three *Sovereign land of …* lines. `+0x08` is a palette index, filled at new game from
`g_realmColour` — ten bytes at `0x004DC1D0`, five `(pen, highlight)` pairs, indexed by the
realm's **shield**. Its five pens are red, yellow, near-black, magenta and blue, which is the
player's list exactly.

**What we did.** `Ink::realm` — a table of six colours we invented, whose own doc comment said
*"Presentation only — which lord flies which colour in the original is not established here"* —
indexed by the **realm id**. So the county strip had a different table *and* a different key
from everything else on the same screen. The minimap tint, the menu-bar banner and the
campaign flag all go through the shield; the strip was the one consumer that did not.

**The key is the interesting half.** A wrong table is a transcription error and a wrong key is
a model error, and this was both. `Realms_AssignLords` (`0x0049CAAA`) walks realms 1 … 5 and
gives each AI the *first unused* shield, the humans' picks having been marked first — so the
human's choice shifts every AI's colour, and no realm id has a colour of its own. The player
said this himself in five words, in the parenthesis: *"(at least, because I picked red)"*. He
was telling us the assignment was contingent on his own choice, which is precisely the
property a table keyed by realm id cannot have.

**How it was settled without a second playthrough.** The eleven `.sav` fixtures are two
different games. In `england-turn1.sav` realm *n* flies shield *n*, so it cannot distinguish
the two keys — and that is the save almost every test on this project runs against. The
battle triple and the turn pair have **realm 1 flying shield 5**, and there realm 1's stored
pen is `0x04`, blue, which is shield 5's. Over all eleven, `+0x08` equals
`g_realmColour[+0x0A]` for 25 of 25 realms, **ten of them with id ≠ shield**. The test asserts
that separating count rather than only the equality, because a check that passes for the wrong
reason on the only fixture anybody runs is exactly the failure this correction is about.

**A `[V]` on the thing I was asked to check, that came back the other way.** The suggestion
reaching me was that the lord's *name* line might take a different pen from the two lines
above it, which would have explained a uniform grey looking wrong. It does not:
`CountyStrip_Draw` computes `colour` once and passes the same local to all three calls. The
grey is the emboss and the realm's colour is the pen, on every line. The fix was one level up
from where it was expected to be, and saying so is cheaper than a change that makes the
symptom go away for the wrong reason.

**The clamp that would have hidden it, and the one place it belongs.**
`chrome::realm_colour` clamps a raw shield to 1 … 5 before using it as a *frame index*,
because there is no such thing as "no frame" — and a zero clamped up to 1 renders as a
plausible wrong colour that survives a canvas diff. A **pen** has an honest answer for "we do
not know this realm's colour", so `chrome::realm_pen` returns `Option` and the caller falls
back to something visibly ours. Same byte, two consumers, and only one of them can afford to
guess.

**The rule.** *A fact the game stores once should reach the screen through one table and one
key.* We had three tables — the minimap ramp, the pen pairs, and `Ink::realm` — for one thing,
and the third existed only because nobody had looked for the second. C5's lesson at the scale
of a palette: the table was already in the binary, already in `symbols.json` with its five
pairs written out, and had been there since somebody read `Realms_AssignLords`. Nothing
connected it to the screen that needed it.

**And the same player, a message later, on how the colours are handed out:** *"the game will
always try to give the Knight yellow, the Countess blue, the Bishop purple/pink — I can't
remember for Baron — and it'll move a noble's colour around if you pick it."* Every colour is
right and the Baron he could not remember is black. The framing is the interesting part,
because it is a **true description and a false rule**, and it took a third reading to see that
the arrow points the other way.

The suggestion reaching me was that this would be one of two things: pure first-unused walked
in *lord* order, or preference-then-fallback. It is neither. `Realms_AssignLords` assigns the
**shield first**, by position — the lowest colour no human has taken, walking realms 1 … 5 —
and then picks the **lord from the colour**, out of `g_lordChoice`, four candidates per shield.
No lord is consulted and none has a preference. A default England game looks like ownership
because group 0's lists lead slot 2 with the Knight, 3 with the Baron, 4 with the Bishop and 5
with the Countess.

The two readings part exactly where he said they would, and neither of the two guesses
survives: **take yellow and the Knight does not move to another colour of his own — he becomes
the black lord, and the Baron becomes the red one**, because red's list names the Baron first
and the walk reaches red before black. Over the five colours a person can take, "the Knight
gets yellow" holds in four and fails in the fifth. That is what makes it a good description and
a bad rule, and it is the shape `docs/rules.md` now carries in both halves.

**He was remembering a real table, and it exists.** `g_battleLordShield` (`0x004D4CA8`) is two
words per lord, `{preferred, alternate}` — Knight yellow else magenta, Baron red else blue,
Countess blue else red, Bishop magenta else yellow — and `FUN_0042BA40` reads it as *"if the
human has my colour, take my other one"*. Genuine preference-then-fallback, three of his four
colours in its first column, and it governs the **custom battle** and nothing else. A player
whose description matches a table that exists but belongs to a different screen is not
misremembering; he is reporting from the part of the game he last saw it in, and the useful
response is to find both tables rather than to pick one.

**What no fixture could settle.** All eleven `.sav` files here have the human on shield 1 or
shield 5 — never a middle colour — so not one of them exercises a collision the readings
disagree about. The eleven-save check that settled the *key* in the paragraphs above is
silent on the *walk*, and saying which of two questions a body of evidence answers is the
whole of not over-claiming from it. The walk is read from the walk; `docs/rules.md` §7a's
table is derived and marked so, and the row a fixture *can* confirm — the England default —
is asserted against `england-turn1.sav` as the one anchor the derivation has.

**One thing this leaves behind.** `l2_scenario::newgame::assign_lords` hard-codes
`shield = realm`, which is the default mistaken for the rule, in code, with a doc comment that
said so as a mechanism. The comment is corrected and the gap is recorded rather than closed:
`NewGame` has no shield field, so nothing can yet pick a colour to break it, and closing it is
the setup screen's work rather than this one's.


**C123 — the sidebar's four grain forecasts are the tail of a function we
ported only the loop of, and a player found it the same afternoon the audit counted it.**

> *"Sidebar doesn't show grain being planted as a negative number."*

He is right, and he is describing **Spring**. `Grain_LabourEstimate` (`0x0044D374`) is a
search loop followed by a tail, and `l2_kingdom::land::grain_labour_estimate` reproduces the
loop, returns `GrainEstimate { wanted, useful }` and stops. The tail writes four things
nothing in this workspace computes — `+0x230` (what sowing will cost), `crop[2]` (the harvest
forecast), `+0x2FC` (the growth forecast) and `+0x22C`, the **signed** number the sidebar's
grain row draws:

```c
if      (season == 1) county.field_0x22C = -county.field_0x230 - county.grainEaten;
else if (season == 4) county.field_0x22C =  county.crop[2]     - county.grainEaten;
else                  county.field_0x22C = -county.grainEaten;
```

In Spring that is `−sown − eaten` and cannot be positive. **So this is not a formatting bug.**
`Ui_DrawDelta` (`0x00402E0C`) is perfectly capable of a negative — it draws `-value` with a
`'-'` lead in `colourNeg`, a `'+'` lead when positive and a blank `'@'` at zero, all as one
`Ui_DrawText` with no separate minus mark — and the number simply never reaches it.

**Two things about this are worth more than the fix.**

**One: the reason for the doubled estimate round was already understood, and the values it
exists to produce were still dropped.** `crate::field`'s module docs say the
`Labour_Allocate` / `Herd_UpdateCrowding` / `County_RefreshEstimates` round runs twice "for …
the panel forecasts, which the estimates fill from whatever the allocator last decided."
These are those forecasts. Knowing *why* a pass exists is not the same as carrying what it
writes, and the port is faithful right up to the line where the interface starts — which is
exactly the seam C30 is about, seen from the drawing side. The forecast is not recoverable
from the estimate either: the loop calls `Grain_Sow(county, workers, grain − grainEaten)` and
the tail calls `Grain_Sow(county, staff, grain)`.

**Two: an instrument on this project got ahead of the player for the first time.**
`docs/draws-map.md` §5.5 counted the eight `Ui_DrawDelta` calls as missing hours before the
report arrived, and §5.10 could answer *which of three things is wrong* by reading rather than
by guessing. Every previous defect on this screen — C57, C58, C60, C61 — was explained after
he found it. `docs/plan.md` §0's row *"a screen showing the wrong thing — instrument: none"*
now has one, and this is the evidence that it works.

**And a second defect on the same row, which is the one that would have been mistaken for
it.** `Ui_DrawNumberRight` (`0x004030C6`) ends in `FUN_004025D7`, which computes
`x + (width − textWidth) / 2`: it **centres**. Its name and its `docs/symbols.json` comment
both say right-aligned. The grain row's *store* is therefore centred in 60 pixels from x 480
and `county::draw_produce_rows` right-anchors it. Found independently by the other draw audit;
recorded here because it lands on this row.

**C124 — a `[V]` claim in a format document produced a player-visible defect,
and the renderer was right to trust it.**

> *"The wheat fields don't show the wheat growing."*

`docs/formats/maps-layers.md` §5.5 said `Terrain_Set`'s third parameter — `variant`, which
shifts the frame by a whole four-frame block — is **dead**, on the grounds that all sixteen
call sites pass zero, *"including the two that forward a parameter (`FUN_00469D21`, whose only
callers are `Grain_SeasonTick` and `Herd_UpdateCrowding`, and both pass `'\0'`)."*
`l2_view::campaign::field_graphic` was written to that and computed
`base + (storedFrame & 3)`, with no variant term.

There are **twenty-four** call sites. Twenty-three pass a literal zero. `Herd_UpdateCrowding`
passes zero. **`Grain_SeasonTick` does not:**

```c
band    = FUN_0044CF6F(county.crop[2], county.fieldsGrain);   /* 2, 3, 7 or 11 */
variant = band < 3 ? 0 : (band - 3) / 4 + 1;                  /* 0, 1, 2 or 3  */
FUN_00469D21(county, band, variant, 2, 0xE);
```

> **The first line of that block is wrong, and it is why the fix below did not reach the
> player.** `Grain_SeasonTick` calls `FUN_0044CF6F` once in each of its three arms: `crop[1]`
> in Spring, Summer and Autumn, `crop[2]` only in Winter, and all three divide by the byte at
> `+0x206`, not `fieldsGrain`. Built as written, the wheat drew variant 0 until the harvest.
> The block is left as it was so this entry still reads as what was believed;
> `docs/decisions.md` C195 has the three calls.

**And the two halves of this bug are not independent, which is the part worth keeping.** All
four density bands fall in `2 … 0x12`, whose base is 88 — so the `content` byte carries *no*
information about the crop's stage and the variant carries all of it. Our season pass also had
no counterpart for that repaint at all, so `content` never moved either; **fixing either half
alone would have changed no pixel**, and a fix aimed at the obvious half would have looked
like a failure and sent the next person somewhere else.

The artwork closes it, and it is the kind of check this project trusts: `Roads1a.pl8` frames
88 … 103 are sixteen 58-wide diamonds, four variants of four variations, and the ripe-gold
pixel count rises strictly with the variant at every one of the four positions —
36/34/36/33, 45/43/46/43, 54/50/55/49, 71/70/72/69 — while the fallow block before (84 … 87)
and the pasture block after (104 … 107) carry two to eleven. Four blocks of four from 88 end
at 103, and 104 is exactly where `field_base`'s next base begins.

**Three things generalise.**

**One — the claim was checked on one of two branches and stated about both.** That is the
*name the branch* rule from `docs/agents.md`, and *"`Herd_UpdateCrowding` passes zero"* is a
finding that cannot be promoted by accident.

**Two — a wrong `[V]` in a format document is worse than a wrong correction, because nothing
warns you.** `docs/decisions.md` carries a standing note that the correction log can be wrong
and is believed harder than anything else. The format documents are believed exactly as hard,
are consulted by more code, and have no such note. This one did not merely fail to help: **it
produced the defect**, through a careful person who looked the reference up.

**Three — an existing test was written to the falsehood and defended it.**
`a_fields_picture_follows_its_crop_state` asserted `frame == base + variant` for terrain
`0x05`, a value the game never writes to a farm tile, and would have gone red on the fix. A
test whose subject is a documented claim inherits the claim's errors, and the tell here was
available: **the value it asserted on was one nothing in the game produces.** A ladder test
that walks unreachable rungs is testing the document, not the game.

**C125 — the correction that identifies a class must enumerate the
class. Naming a category and fixing one member of it is the most expensive kind of
half-finished work, because the name makes it look finished.**

An hour after *"rations slider moves but is inoperable"*, the same player: *"'People pay 0
crowns' on the tax thing always says 0 crowns. And the happiness bonus/minus on the tax screen
is also stuck and not adjusting."* Two symptoms, one missing call, and it is the one that had
just been added one door along.

**What the original does, arm by arm**, because the player also asked *"have we compared our
functionality to the binary?"* and the answer should be written where the next panel can use
it:

```c
Tax_Increase (0x0043AA32)          if (taxRate < 0x32) Tax_IncreaseCounty(sel);
Tax_IncreaseCounty (0x0043AA83)    taxRate++;  Tax_RecomputePreview(county);  Panel_Tax();
Tax_RecomputePreview (0x0044B80B)  taxShown     = Pct(Pct(population, castleMult), taxRate);
                                   dHapTaxLocal = 5 - taxRate;
                                   taxHapOther  = g_taxHappinessOther[taxRate];
                                   Tax_SumEmpireHappiness(owner);
                                   FUN_0044BA35();          /* the empire-wide sum of taxShown */
Panel_Tax (0x0041152F)             draws exactly those three, plus the realm's empire term
```

Ours wrote `tax_rate` and returned. `tax_shown` had **one writer in the whole tree**,
`tax::collect`, which runs in the season pass — so it is zero until the first collection and
afterwards describes last season's rate.

**And the other half is a new species of the documented-and-absent bug.** Both happiness terms
had `tax::recompute_preview`, whose own doc comment says it runs *"inside
`County_MakeIndependent`, `County_SetOwner` and every tax control"* — and **no tax control
called it.** `Unit::mission` and `County::farm_style` were the first two instances and both
were *fields nothing wrote*; this is the first on a **function nothing calls**, and the two are
not equally survivable:

> **A field nothing writes is invisible. A function nothing calls has a comment claiming
> otherwise** — and the comment is what carries it through review, because a reader who opens
> `recompute_preview` finds an accurate account of when it runs and no reason to check.

The remedy is the same one this project keeps arriving at from different directions: the claim
*"this runs from every tax control"* is a **second artefact**, and it agreed with nothing.
Grepping the callers of a function whose comment names its callers takes ten seconds and is
now the habit — it is the same act as reading a decompilation against a claim rather than for
it.

**The generalisation is the finding.** The ration correction identified a category — *a
control in this game recomputes and repaints; a setter that only sets is not the control* —
and then fixed one instance of it. The tax panel was three feet away, has the same two arrows
in the same widget table (`g_taxWidgets`, `0x004DD790`), and was broken in the same way. The
right move after C118 was to grep for the other setters, and the reason
it did not happen is that the correction read as *finished*.

So, the sweep that should have run then, run now — every `Game::set_*` against the original's
control for it:

| ours | the original | recomputes? | state |
|---|---|---|---|
| `set_ration_split` | `Ration_SetSplit` | food pass, search, allocate ×2, repaint | fixed |
| `set_tax_rate` | `Tax_IncreaseCounty` | `Tax_RecomputePreview`, repaint | fixed here |
| `set_ration` | `Ration_IncreaseCounty` (`0x0043A23F`) | `Ration_Apply`, `County_RefreshEstimates`, repaint — **once each, not twice** | fixed, and found by this table |
| `set_industry_share` | `FUN_00439122` | allocate ×2 — already done | correct |
| `toggle_industry` | `Industry_ToggleFromMap` | allocate ×2 — already done | correct |

**`set_ration` was the row that paid for the table.** It was filed *open* rather than
*believed fine* — an unread member of an enumerated class is a known unknown, and this project
has repeatedly been bitten by the other kind — and reading it the next morning took ten
minutes and found the same defect a third time. Nobody reported it; there is no player
sentence for this one, because the enumeration got there first. That is the whole argument for
enumerating: **the third instance was the cheapest to find and would have been the most
expensive to have shipped**, since by then the pattern would have looked like a fact about our
architecture rather than three copies of one omission.

Its one asymmetry is kept: the level change runs `Ration_Apply` and
`County_RefreshEstimates` **once each**, where `Ration_SetSplit` runs them twice. That is the
original's, and the reason is legible — the split's search can leave the county's labour
describing a split it walked away from, and a level change cannot.

**One half of the report is not a defect and is now asserted so.** `taxHapOther` is
`g_taxHappinessOther[rate]` and that table is **flat zero from 0 to 19**. Over the range a
player actually uses, the *Other counties* line does not move and the panel is right. C26
recorded the flatness; what it did not record is that this makes the line *look* identical to
the genuinely-stuck one beside it, which is why one sentence reported both.
`the_empire_tax_happiness_term_is_flat_until_the_rate_reaches_twenty` pins it so nobody
"fixes" it, and `docs/rules.md` tells a player why the two lines behave differently.

**And a question `docs/kingdom.md` §1.3 asked and gave up on, answered in passing.** It lists
`taxShown` and `taxCollected` together and says it does not know how the two ever differ.
`Tax_RecomputePreview` has **no suppression test**; `Tax_Collect` zeroes the base when the
county's tax is suppressed. That is the whole of it: a suppressed county goes on telling the
player what his people *would* pay while the treasury banks nothing. `[D]`.

**What was checked and is not wrong.** `Panel_Tax` draws with `Ui_DrawNumber` and
`Ui_DrawCount`, not `Ui_DrawNumberRight`, so the centring correction does not reach this
panel. Eighteen call sites elsewhere remain unaudited; that is a separate sweep and this is
not part of it.

**The trap that was watched for and is not present.** The ration path has `ration::preview`
and `ration::apply` — same name as the original's single `Ration_Apply`, opposite behaviour on
the store — and reaching for the wrong one would have had a drag eat the county's herd a
hundred times. The tax path has no such pair: `recompute_preview` computes and `collect`
banks, the names say which, and only `collect` credits a realm. Recorded because *looking and
finding nothing* is the half of a check that usually goes unwritten.

**C126 — The narrator is 84 % of the game's audio, and one trigger reaches all of him.**

A player, on being asked what he was missing: *"that guy's voice acting is half the
personality of the game."* Measured against his install, he understates it. **646 of the
771 shipped `.wav` files are somebody speaking** — 449 lord takes and 197 system clips —
against 10 music tracks and roughly 70 effects. The voice is not a feature of this game's
audio. It is the audio.

We now reach **543** of them, from 0. The change is small and the number is large for one
reason worth generalising: **the voice hangs off a single trigger, and the trigger is a
table lookup rather than a constant.** `Msg_PlayVoice` (`0x004B35C1`) maps `(group,
variant)` to a filename through four tables; every other sound primitive in the binary is
called with a literal or a slot number. So sixteen of the 134 trigger sites carry 543 of
the 771 files, and the other 118 carry 12.

**That is a criticism of every 1:1 count this project keeps.** Input arms, draw calls and
sound triggers all weight their rows equally, because equality inside a list is what makes
a denominator meaningful. A player weights them by what he notices, and the two orderings
are not close. `docs/audio-triggers.md` now carries a second column saying what each row
*carries*; it cost one column and it changed the work order. The count stays the
denominator — it is still the only thing that cannot be argued with — but it should not be
the only number quoted.

**The mechanism, which is the part that would have been guessed wrong.** `Msg_DrawWindow`
(`0x0047309E`) is 10,915 bytes and is not a painter: it dismisses, enqueues, sets its own
timer and plays its own sound from inside the draw. There is no call site to put a voice
beside. All sixteen `Msg_PlayVoice` calls are guarded by `g_messageTimer == <constant>`,
and the timer counts **down** from 2000 at one per tick, so the trigger is a *countdown
reaching a value*. Five constants, and they are one rule and a delay:

* `0x7C6` is 1990 against a start of 2000; `0x5A` is 90 against the tip's clamped 100.
  **Both are ten ticks after the window opened** — the tip needed a constant of its own
  because its timer starts differently, not because its rule differs.
* `0x776` (90 ticks), `0x76C` (100) and `0x708` (200) belong to exactly the categories that
  play a **fanfare** on the opening frame. The voice waits for the trumpet rather than
  talking over it, and the longest wait is the one that also plays a lord's sting at 90.

Equality rather than a threshold is what the port uses too, and it is the better shape
here: the timer decrements by one per tick, so each value occurs once per window, a `==`
fires exactly once, and there is no memo to keep and nothing to reset when a window is
dismissed early.

**Two artefacts said this was already recorded and neither was.**
`crates/l2-game/src/screens/message.rs` lists the five constants and says they are
*"recorded in `crate::message`"*. They had never been written there — a citation that does
not resolve, which is a rule with no way in wearing a doc comment, and it survived because
the sentence reads like a hand-off. And **which category takes which constant** was not
recorded anywhere, which is the only half a caller actually needs. Both are fixed at
`audio::voice_tick`.

**Three corrections fell out of reading the function.**

* `names::system_voice` accepted `200 ..= 299` and `g_msgVoice200` is 85 entries,
  `200 ..= 284`. Fifteen invented groups. `docs/symbols.md` was right; the install settles
  it independently, since the highest `S2xx` file that ships is `S284_02.wav`.
* **The narrator reads a notice as a *chain*.** `FUN_004B3ACD(group)` walks a five-wide
  table at `0x004E1E40` and plays `S201_02.wav + (n − 1) × 0x10` one clip at a time as each
  finishes, gated on `Sound_OneShotBusy()` and a 1,000 ms gap. That is why `S010_13.wav`
  exists. We play `_01` and stop, and that is now written down as a gap rather than left to
  look like completeness.
* The first enumeration of sound trigger sites counted 121 and missed `FUN_004262cf`
  entirely — a 28-byte forwarder onto `Sound_PlaySlot` with 13 callers of its own, all
  battlefield sounds. A wrapper is what a single-pass grep loses.

**And one thing that worked.** `audio::before_the_campaign` is exhaustive over `ScreenId`
with no `_` arm, so that adding a screen cannot compile until somebody says whether music
plays behind it. The message window was the first screen added after it landed, and the
branch that added `ScreenId::Message` had to answer — and left a comment saying why a
message never changes the music. That is the *"make the mistake unrepresentable"* pattern
paying out on its first real encounter, in a place where the failure would otherwise have
been silent for weeks.

**C127 — every number in the game reserves a leading column, and dropping it put
three sidebar figures four pixels left.**

> *"Happiness # and population # in the sidebar are slightly left of where they should be —
> not sure if we've compared that to the draw in the original or what makes it off."*

`Ui_NumberToBuffer(value, 1, 0)` writes the digits from index **1**, leaving index 0 for a
sign, and `Ui_DrawNumber` fills it from its `lead` argument before drawing the whole buffer at
`x`. So the string at `0x1FC` is `" 435 "` and the **digits** start at `0x1FC + 4`. We drew the
bare digits at `0x1FC`.

**The column is deliberate and the binary says so 62 times.** Across `Ui_DrawNumber`'s 190 call
sites (191 as first written, which counted the definition line; C163) the lead is `' '` 115 times, **`'@'` 62 times**, and `'+'` and `'-'` once each; never
`'\0'`, which the function treats as *terminate immediately*. `'@'` is a glyph with no picture —
an invisible sign column that still holds its place — and asking for one 62 times is only
meaningful if numbers are meant to align on it. `crate::shell::font::SPACE_ADVANCE`'s doc
comment had already said exactly that; the call sites simply did not use it.

**Two things worth more than the four pixels.**

**One — the report contained its own discriminating test, and the obvious cause was wrong.**
`Ui_DrawNumberRight` had just been found to *centre* rather than right-align, which fitted the
symptom and was the first thing to check. It is not the cause: right-anchoring would displace a
two-digit happiness *further* than a three-digit population, and a lead displaces both by the
same four pixels whatever the value. These two calls are `Ui_DrawNumber`, which has no
anchoring argument at all. **When two causes fit a symptom, look for the one that predicts a
different *pattern* rather than the one that predicts the same sign of error.**

**Two — this is the draw-call inventory's `arms.json` moment.** All three figures sit inside a
row `docs/draws-map.md` scored **18 of 18 reproduced**, because `reproduced` means *we make a
corresponding draw* and never meant *where the original puts it*. That is precisely the gap the
input inventory has between *the arm exists* and *the arm is the right gesture*, and it wants
the same third verdict: `reproduced` / `placed` / `absent`. **Until that pass runs, 59 of 121 is
an upper bound on fidelity and a fair count of coverage, and the two must not be quoted as one
number.**

The measured sample, reported as one sample because extrapolating it would be the same sin:
**of the eighteen draws read back against their call sites, fifteen were placed and three were
displaced.** Not zero. And nothing but a person reading was ever going to find it — all three
passed every test in the tree, and **two of those tests asserted the wrong coordinate while
correctly quoting the call site it came from**, which is the sharpest form of the hazard:
*the call site's `x` is not the picture's `x`.*

**C128 — four causes fitted, the binary picked one, and a name in our own
records file had generated a second.**

> *"I right now have −11 cattle. If I move it so the people are eating cattle, it still says
> −11 cattle in the sidebar."*

Four candidates were separable and all four were put to the binary rather than to our code.

**1. Does the figure exclude slaughter? No — and the game's own labels settle it.**
`Herd_LabourEstimate`'s tail writes `herdOverallChange = (births − deaths) − herdEaten`.
`Panel_JobCattle` (`0x00413B30`) draws **three** lines out of `L2.eng` group 77:

| label | index | value |
|---|---:|---|
| *"Change due to farming"* | 7 | `birthsExpected − deathsExpected`, **computed inline and never stored** |
| *"Change due to eating"* | 27 | `−herdEaten` |
| *"Overall change"* | 28 | **this field** |

**`docs/records.json` named `+0x258` `herdChangeFromFarming` and cited index 7.** It is index
28. Renamed `herdOverallChange`, 303 record fields before and after.

That wrong name is not a footnote: **it is what made "the delta excludes slaughter" the leading
hypothesis**, in a brief written by someone who had read the file. Third time in two days that a
document acted as an *input to reasoning* rather than a record of it — after `maps-layers.md`'s
dead variant produced the static wheat and `symbols.json`'s "right-aligned" produced a
misplaced number. The pattern is worth stating once: **a wrong name in a data file is repeated
by everyone downstream and interrogated by no one**, because a name is not a claim anybody
thinks to check.

**2. Is it recomputed when the split changes? No, and that is the bug.** `Herd_LabourEstimate`
is one function — a search loop that fills the cattle ceiling, then the tail — and the original
calls it from **both** `Herd_SeasonTick`'s last line and `County_RefreshEstimates`. We had the
loop in `field::refresh_estimates` and the tail in `land::herd_season_tick` alone, so the
forecast moved once a season and no control could move it. `herd_preview` now runs in
`refresh_estimates` beside the estimate, exactly as `grain_preview` does.

**This is the same split, in the same file, for the third time in one evening** — grain's
forecast, the cattle forecast, and (harmlessly, so far) `Herd_UpdateCrowding` missing from the
middle of `Kingdom::refresh_estimates`'s doubled round, which `field.rs`'s own module docs
describe correctly two hundred lines above the code that omits it. **The loop is the part that
looks like the function and the tail is the part the interface reads.**

**3. Are our births wrong? No.** `herd_growth` matches `Herd_BirthsAndDeaths` constant for
constant: bands `(10, 1, 1400) (20, 3, 900) (30, 5, 500) (40, 7, 200)`, staffing
`PctOf(labour, herd*3)` capped at 200, understaffing `(100 − staffing) / 3` **added to the
death rate**, the small-herd bonus `+10000 / +5000 / +2000` below 5 / 10 / 25 head and gated on
full staffing, and the season multipliers.

**One correction to the numbers quoted at me, because it changes what is plausible:** at low
crowding deaths are **1 % of the herd, not 0.01 %** — the rate is per-ten-thousand and is
applied to `herd × 100`, not to `herd`. Births at low crowding and full staffing are 14 %.

**4. Is a −11 at low crowding representable? Yes, two ways, and neither is a bug.**

* **Slaughter.** At low crowding and full staffing the natural net is `+13 %`, so a herd of ~100
  with ~24 head eaten gives −11 exactly.
* **Understaffing, which is invisible to crowding.** Crowding is `herd / fieldsCattle`;
  staffing is `labour / (herd × 3)`. **A large pasture with few milkmaids reads *low crowding*
  and is *badly understaffed* at the same time** — the two axes are independent, and at zero
  staffing the death rate is `1 + 33 = 34`, i.e. **34 % of the herd**, with births at zero
  because the small-herd bonus needs full staffing. Double-digit losses in the best crowding
  band are the rules working.

**And one thing the reports do not agree about.** *"Only getting 1 cow"* with *"lots of
milkmaids"* is inconsistent with *low crowding*: at low crowding and full staffing births are
14 % and a herd small enough to yield 1 would collect the small-herd bonus and yield 3 or 4.
Births of exactly 1 fits **band 4** (rate 200, so 1 calf at 50 … 99 head), the worst band.
Two anecdotes from possibly different counties should not be fused into one model — so the
question to put back is *which county, and which crowding line did the panel show*, rather than
a fifth candidate.

**`docs/rules.md` owes a line either way**, because *"Cattle, and change next season"* invites
the whole-change reading and the whole change is what it is — while the panel behind it splits
that into three, and only the third matches the sidebar.

**C129 — the third unwritten tail in one evening, and the three of them were
one fix rather than three.**

> *"The figure is missing in the sidebar — it draws the serf reclaiming, but not the +1 I'm
> used to."*

`Field_ReclaimEstimate` (`0x0044C278`) is a work-outstanding loop that fills the reclamation
labour ceiling, **plus a tail** that simulates the coming season: it hands `labour[2].workers`
out from the nearest-to-finished field, wrapping the twenty slots, 200 units per field, and
counts each field that crosses 800 into `+0x20C`; then it divides the lead field's remaining
work by the **full** staffing, rounded up, into `+0x214`. `reclaim_labour_estimate` ported the
loop and stopped.

**So the "+1" is a count of fields that will be *finished* next season** — fields, not units of
work, which a small integer could equally have been. And it is a simulation rather than a
division for one specific reason: **a field that finishes hands its surplus to the next**, so a
big enough gang completes two in a season, which is the only way the figure ever exceeds 1.

**The generalisation is the valuable part, and it changes the shape of the remaining work.**
Cattle, grain and reclamation were **the same defect three times**: three estimate passes that
`County_RefreshEstimates` calls, each a search loop followed by a tail, each ported as far as
the loop. One fix repeated, not three investigations — and the fourth instance of the same
split, `Herd_UpdateCrowding` missing from the middle of `Kingdom::refresh_estimates`'s doubled
round, is sitting there harmlessly today.

> **When a function is a loop followed by a tail, the loop is the part that looks like the
> function and the tail is the part the interface reads.** A port that stops at the loop
> compiles, passes, and is invisible until somebody looks at the screen.

**The four industry rows are *not* the same fix**, and this is the thing to write down so
nobody batches them in. Each reads an `i32` at the head of an `Industry` record, and the rows
for stone, wood, iron and weapons read `industry[2]`, `industry[4]`, `industry[1]` and
`industry[3]` — **each the record above the commodity its own row draws, and the wood row's
`0x2F0` one whole record past the end of a four-record array.** Either the original is off by
one in all four or `docs/records.json`'s `Industry` base is wrong; nothing settles it. Four
confidently wrong numbers on screen would be worse than four blanks, so they stay blank and the
record layout is the prerequisite.

**C133 — we read these panels' numbers out of the binary and wrote their
words ourselves, and the words are where the game explains itself.**

A player: *"Also sorely missing: 'All your people are fed by dairy.'"*

**That string does not exist.** Every one of `L2.eng`'s 317 groups was searched for *dairy*,
*fed by* and *all your people*: the hits are group 8's *"Dairy maid"*, three event texts, one
tip, and **group 62** — *"No dairy produce"*, *"Dairy produce feeds"*, *"RATIONS MET."* —
which is the ration screen `docs/rules.md` already records as **cut**, with a food-priority
mechanic the shipped game does not have. So this is the third remembered detail from him
tonight that does not survive checking, and the third time checking was cheap. He is right
about the *behaviour* every time and the discipline holds: **a memory that fits is not
evidence.**

**But the report was still right about the panel**, which is why this entry exists rather than
a one-line reply. `Panel_Ration` (`0x00411B72`) is the **only consumer of group 87 in the whole
binary** — enumerated, not assumed — and it draws seven of that group's twelve strings. Our
panel drew **none of them**. Its whole vocabulary was hard-coded in our own words:

```rust
mod g87 { pub const TITLE: &str = "RATION"; pub const WANTED: &str = "WANTED:"; … }
```

and so is the tax panel's, the population panel's and the happiness panel's. **The numbers
were treated as the mechanism and the text as a skin over it**, and that is exactly backwards
for a game whose panels explain their own rules in words. It is a habit rather than an
oversight, and it is the honest answer to the player's standing question *"have we compared
our functionality to the binary?"*: for these panels, we compared the arithmetic and not the
sentences.

> **A screen's strings are part of its specification, not a skin over it.** A group with one
> consumer *is* that screen's vocabulary, and reading the painter without reading the group is
> reading half the function.

**The count, the way the draw audits give it.** `Panel_Ration` makes **26** content draws — 24
unconditional and 2 more when *Armies eat* is on. Before this branch we made **12** of them.
Now **18**, and the six added are the `Misc_cty` frames that say what each column is; without
them the Fed and Eaten rows are unlabelled numbers, which is most of what *"no information
about feeding peasants is available"* actually meant. **Still missing: the two *Armies eat*
draws** (`+0x19C + +0x198` and 87/8, *"men foraging in the county."*), which need a field pair
we do not carry.

**And five of group 87's twelve strings are drawn by nothing at all** — 6 and 7, both
*"Feeds"*, and 9, 10, 11: *"growing"*, *"harvested"*, *"planted"*. Same shape as
`docs/bugs.md` B82 and B83, established the same way, and filed there.

**What this does not change.** `docs/rules.md`'s caveat — that a player cannot tell his county
is fed entirely on dairy except by reading the Fed row — **stands**, because the panel says it
with a *number* and not a sentence. That number is the Fed row's third figure,
`herd × dairyPerHead`, which C120 added an hour earlier: when it equals the
population, all your people are fed by dairy. So the player was asking for a readout that does
exist, in the form the game actually uses, and which we had just started drawing. The two
findings are one condition with two readouts — and wiring them together is what makes the
panel able to answer the question that started this whole thread.

## Open questions

- **`County.purse` on an unowned county has never been non-zero in any game we can drive.**
  Not in a hundred turns of England, not in a hundred of a fourteen-county empire, not on
  any of the forty-four shipped maps. It needs the save `docs/oracle-requests.md` §6 asks
  for, and it is the longest-outstanding oracle request in the project.
- **Bankruptcy has never fired.** `Realm::bankrupt_stage` reached 0 — not 1 — in every run
  above. The AI lords are handed free gold every turn and never overspend it, and an
  unplayed human raises no army, so nothing we can drive ourselves ever misses a wage.
  Six of the ladder's messages are therefore unreachable code by construction rather than by
  defect. `docs/oracle-requests.md` §3.
- **The empire tax term is a human-only mechanic in practice.** The highest tax rate any AI
  lord set in a hundred turns of England is **12**, and `TAX_HAPPINESS_OTHER` is flat zero
  below 20. So the whole table above its first row can only be reached by a person, which
  makes `docs/oracle-requests.md` §1 the only route to evidence about it — a played game of
  ours cannot generate one however long it runs.
- **The difficulty curve 116/108/100/92/84 rests on the decompilation alone.** Making the
  shipped `TROOPS*.ENG` an oracle for it was tried and does not work: the non-Normal rows
  are hand-authored leftovers (402 of 3,080 populated in `TROOPS.ENG`, ratios running 1.20
  to 2.00 with 116 % nowhere among them) which the engine overwrites. `docs/formats/eng.md`
  §3.2 said those rows were "all zeros" and that is corrected there. The percentages are
  immediates in the caller of `FUN_00404D6B` and would need `initconsts.ps1`'s technique to
  recover.
- ~~**The labour allocator never reruns.**~~ **Closed.** It is
  `Pass::LabourAllocate` and `Pass::LabourAllocateAgain` now, behind the six estimate tail
  calls it needed; `sum(labour) == population` holds for all fourteen counties of the England
  position for ten seasons, and `ten_more_seasons_...` asserts that instead of the freeze.
  The castle ceiling's materials gate was the last piece still inferred and it is closed —
  see C63.
- ~~**The castle's materials are debited up front, and the original delivers them over
  time.**~~ **Closed, and it was a defect rather than a simplification.** The six words at
  county `+0x1CC … +0x1E0` are on `County` now, `Castle_DeliverMaterials` (`0x00450CCD`)
  carts them in season by season, and `Castle_BuildEstimate`'s gate shuts. C63.
- ~~`WEATHER_JITTER_BOUND` in `crates/l2-kingdom` is **invented**.~~ **Closed, and it was
  closed twice before anyone noticed** — see C91. `Rand_Advance`
  (`0x00404A46`) publishes `g_rand7B = g_randStateB & 0x7F` and `Weather_UpdateAll` divides
  it by 8, so the draw is 0…127 and the jitter 0…15. The constant is 128 and is right. What
  *was* still open in the same paragraph, and is now closed too, is `localModifier` —
  `FUN_00449D6E`, `docs/kingdom.md` §7.3.
- The four map planes whose meaning is inferred rather than proven (graphics bank,
  descriptor index, multi-tile object part), and the exact tile → lattice mapping,
  whose best affine fit reaches only 72%.
- 23 files that use supported encodings but fail the end-offset invariant, pinned in
  `KNOWN_FAILING`.
- ~~`Font_c2.pl8` declares RLE but its frames occupy exactly `width × height`.~~
  **Closed**, and it had been closed for a long time: `docs/formats/pl8-failures.md` §5
  settles it, `docs/audit.md` F21 flagged this very bullet as stale, and nobody struck it.
  Leaving it here is not harmless — `crates/l2-view/src/text.rs` cited it as the reason the
  interface drew its own letters, and a third of our screen modules were written in a 5 × 7
  debug font on the strength of that. C107.
- `Title.pl8` decodes with correct geometry but no shipped palette colours it.
- The type-4 apex pair, where the stored data and the shipped blitter disagree.
- PL8 header fields at 0x04, 0x06, 0x07.

**C122 — The message queue cannot be simulation state, and the binary is
what says so.**

`docs/netcode.md` asks every new piece of state which side of the lockstep line it is on, and
a message *queue* looks like the simulation's: the rules fill it, it is ordered, and it
outlives a frame. It is not, and the argument is one line of `Msg_Enqueue` (`0x00472BC5`):

```c
enqueue = (to == 0) || (to == g_localPlayer);
```

All three of that function's `isHuman` branches compute that same predicate before the record
is copied into the ring. **A letter addressed to realm 3 is never put in realm 1's ring at
all**, so two peers of one game hold different rings by construction — not by drift, not by
timing, but because the filter reads `g_localPlayer`. A ring inside
`Canonical::hash_of(kingdom)` would desync every network game on the first letter an AI wrote
to somebody.

So `l2_kingdom` keeps *producing* `diplomacy::Letter` values, which are identical on every
peer, and `Game::messages` — beside the levy and the live battle — is where one peer's copy
of them lands. It is in the **save** (`l2_game::save`, which is per-peer) and out of the
**digest** (`l2_kingdom`, which is not), and those are different files for exactly this
reason.

The general shape is worth keeping: **the question is not "is this state durable" but "would
two peers compute it identically"**, and a filter on `g_localPlayer` answers it before any
reasoning about what the data means.

**C113 — A field that was always empty stopped being always empty, and
the save format had a note saying so.**

`l2_game::save` version 2 wrote the ending messages as a list of their own, under the comment
*"The queue is empty at every point a person can save — `turn::end_turn` settles it — but it
is written anyway."* The comment was true and the reason it was true was that **nothing
displayed the messages**: `Campaign::settle` ran the whole queue instantly at the end of the
turn, with no window and no click.

Building `Msg_DrawWindow` made it false. Endings are now pulled off the ring one at a time and
settled by being dismissed, so a person really can save on the campaign map with three
obituaries queued behind the one on screen — and a save that dropped them is a save that can
never be won. Version 4 stores the whole ring and the record on screen with its timer.

Two things to carry:

* **A "this is always empty" note is a claim about the rest of the system**, and it goes stale
  when the rest of the system changes rather than when the file does. The note was correct, was
  written by someone careful, and named the exact reason it was correct — which is what made
  it possible to notice that the reason had gone.
* The field was written anyway *because* a silently dropped field is a field somebody loses a
  game to. That instinct was right and cost eleven bytes.

**C114 — An arm filed as `missing` cannot run, and two enumerations from
different directions are what found it.**

`docs/arms.json` carried `0x0042FF10/map-message-scroll-dismiss` as **missing**: guard 7 of
`Screen_FrameInput`'s screen-`0x00` arm, *"a right release with a message scroll up dismisses
it and swallows the click"*. It is unreachable. `Screen_FrameInput`'s ladder is

```c
iVar2 = Msg_HandleInput();                       /* 0x0047685D */
if ((iVar2 == 0) && (Screen_HandleInput() == 0)) { …the fifty arms, guard 7 among them… }
```

and `Msg_HandleInput` returns 1 for exactly `g_messageGroup != 0 && g_mouseRightReleased != 0`
— which is guard 7's own condition, tested earlier, with the same `Msg_Dismiss` behind it. So
reaching guard 7 requires the negation of its own predicate. Neither global can move in
between: `g_messageGroup` is written only by `Msg_Pump` (one caller, `Battle_Frame`) and
`Msg_Dismiss`, and `g_mouseRightReleased` only by the per-frame sampler `FUN_004B191E`.

Nothing is lost — it is a duplicate of an arm that already ran — and nothing should be built
for it. It is filed `dead` now.

**What generalises is how it was found**, and it is the same method as the `0x28` battlefield
screen: the arm inventory was built by reading each screen's handlers, and this one was
found by reading the *message* system's handlers instead and noticing that the two lists
overlapped. **One enumeration is a claim; two from different directions is evidence** — and
the direction that finds a dead arm is never the direction the arm is filed under.

**C130 — The picker worked, and every step after the click was
missing.**

A player: *"I picked a colour and it didn't get honoured once the game opened."*

Setup page 4 drew five shields, hit-tested them, highlighted the one you clicked and stored it
in `SetupScreen::shield`. **Nothing read that field.** `l2_scenario::newgame::assign_lords`
took the slot and the lord count and computed the colour as `let shield = realm`, and
`crate::scenario::new_game` then wrote `g_realm_colour` from the realm id it had just been
given back. So the choice reached a field, was painted, and stopped.

The line had a doc comment above it. It said:

> *"the colour slot is the realm id, because `Game_SetupRealms` seeds `shieldIndex = i` and
> only a custom game's colour picker permutes it."*

That sentence is **a default written up as a mechanism**, and it is the whole of why this
survived. The seed is real — `FUN_0049C995` does set `shieldIndex = i` for realms 1 … 5 — and
the conclusion drawn from it is false twice over: `Realms_AssignLords` overwrites the seed for
every AI on every run, and "only a custom game's colour picker permutes it, which this build
has no screen for" was describing a screen that was on the title menu two clicks away.

An earlier agent found the wrong reading and **corrected the comment without closing the
hole**, leaving `// the default arrangement only — see this function's note` above the line and
a paragraph explaining that the gap was *"latent rather than live: `NewGame` has no shield
field at all, so nothing can yet pick a colour to break it."* Read from the world builder that
is true. Read from the screen it is not, and the screen is where the player was. That paragraph
was propagated verbatim into `docs/rules.md` §7a and `docs/mechanics.md`, so three documents
agreed that nothing could ask for a colour while the thing asking was on screen.

> **A gap called *latent* is a claim about every caller, and it is usually made by reading
> only the callee.**

That is the sibling of *a correct explanation sitting directly above the omission it
describes* (`docs/agents.md`): both are prose that is accurate about the function in front of
you and wrong about the system. The difference worth noting is the direction of the error.
That entry is about knowing why a pass exists and not carrying what it writes — a producer
read from the producer's end. This one is about knowing exactly what a value does and not
asking **who supplies it**. Same asymmetry, other end.

**What actually closed it**, and none of it was hard:

* `NewGame::shield`, carried from the screen through `crate::scenario::new_game` beside the
  seed and the head count — *not* on `setup::Settings`, because `Campaign_LoadEntry` rewrites
  all twelve of those from the campaign row and would have silently wiped the colour on the
  one route into a game that does not press the custom page's *Start*. There is a test for
  exactly that route, and it is the only one that fails if the field is moved.
* `assign_lords` written as the real walk, returning shields **and** lords in one struct so a
  caller cannot take the lord without the colour it was chosen from.
* Four tests that click a shield. **Nothing had ever clicked one.** Page 4 was reachable —
  three campaign tests press its *Continue* — and the five hotspots beside that button had a
  painter, a hit test and no test at all, which is `docs/agents.md`'s *"a field is only tested
  if something a test reads was written by something the game runs"* with the writer present
  and the reader absent.

And one measurement worth keeping, because it is the strongest evidence the walk is right and
it is not one of the new tests: `crates/l2-scenario/tests/newgame.rs` builds England from
`L2_maps.dat` and from `england-turn1.sav` and now diffs the **shields and the lords** as well
as the land. That is `Realms_AssignLords` checked against a game the original program set up,
rather than against our own reading of the same two tables. It can only confirm the default
row — every `.sav` this project keeps has the human on shield 1 or 5 — and it is the only row
any file on this machine can confirm.

**C131 — We reproduce *which* gestures a screen answers and not *what kind*
each one is, and 151-of-211 could not see the difference.**

A player reported three things in one breath — the yes/no gauntlets fire instantly where
the game waited and depressed visibly, and holding a spinner does not accelerate — and
every arm he named was on file as `reproduced`. It was not three bugs. `docs/arms.json`
had 211 records whose entire vocabulary was *does this arm exist*, and no field for
press-versus-release, auto-repeat, or a pressed frame.

**The original has one answer and it is a byte.** Two hit-testers — `Widget_Test`
(`0x0040DA1E`) and `Hotspot_Test` (`0x0040E3EE`) — walk arrays of 24-byte records and read
a **kind** at `+0x0F`. Five kinds: hotspot 1 press, hotspot 2 press-then-every-320 ms,
hotspot 3 **release**, widget 4 press-with-accelerating-repeat, widget 5
**press-now-act-in-twenty-frames**. `docs/input.md` is the whole model. Three things fall
out of it:

* **One mechanism serves every spinner in the game.** Armoury, supplies, divide, gift,
  tax, rations, castle, siege, save/load scroll — all `Widget_Test` kind 4, one 30 ms
  clock and one hand-authored 48-byte ramp at `0x004D2748`. There is no second
  implementation to find.
* **The gauntlet is kind 5**, and its twenty-frame delay with the button visibly down is
  what reads as *"the game waited on mouse-up"*. It is not a release at all.
* **`Ui_OkButtonClicked` (`0x0040E7E4`) genuinely is a release**, on all twenty-six of
  `Screen_FrameInput`'s calls, and all four of ours answered on the press.

**The schema change, and why it is not a second taxonomy.** `gesture` already existed and
had drifted into three different things: a kind (`left-press`), a *position in a table*
(`button-0` … `button-5`, fourteen records), and not-a-gesture (`draw`, `timer`). It is
now a **closed vocabulary of the original's own kinds**, the marker in the code carries
it, and the check is set equality on **(id, gesture) pairs** — so an arm answered with the
wrong kind stops counting as reproduced.

**The half that matters is the third check.** The first two compare two artefacts one
person maintains in one sitting, which `docs/agents.md` names as the pattern that lies. The
third reads the **kind byte out of the player's own `Lords2.exe`** and classifies 39
records from it. It found three wrong the day it was written, and it is install-gated, so
it is silent — not green — where it cannot see. Its blind spot is arms dispatched from
`Screen_FrameInput`'s ladder rather than from a table, and **that is exactly where all four
wrong `Ui_OkButtonClicked` arms were.**

**C132 — A removal recorded in the input inventory left its other half in the
painter, and the inventory is keyed on input.**

The campaign map's county-selection arm was removed as an invention, correctly, and
recorded. Its **visual** half — the yellow outline round the selected county — stayed, and
a player reported it four merges later: *"still a weird yellow outline around the county
that is selected on the real map."*

Nobody was careless. `screens/map.rs` carried the comment *"Ours: the selected county
outlined on the shape the player can see. The original has no such outline"*, accurately,
directly above the draw call, and the module header listed it under *"ours, and it should
look it."* This is the shape `docs/agents.md` records as *a correct explanation sitting
directly above the omission it describes*, and the specific mechanism is worth adding to
that list: **`docs/arms.json` is keyed on the input ladder, so an invention whose surviving
half is a *draw* is outside the set it is exhaustive over.** The removal was recorded in the
one instrument that could not see what was left.

There is now an `ours/map-selected-county-outline` record with gesture `draw` and
`removed: true`, and the assertion that could not exist while the outline did:
`the_selection_is_not_drawn_on_the_map` requires the whole 480 × 480 map area to be
**byte-identical** under two different selections — a stronger claim than *the outline is
gone*, because any future selection paint fails it. The two counties it compares are
derived rather than named, because the field markers under `brush` are drawn for the
selected county when the player owns it and are the visible half of a *different*
invention that is deliberately kept.

**C134 — Half of `Unit_StepOnce` was missing, and the tests that
noticed were read as fixtures.**

A player, on build `3F9C11E`: *"The merchants don't move right when you click End Turn, and
then… move insanely fast."* Both halves are one number. Measured through the real screen
machine on the England fixture, one `Machine::update` a frame: the turn was **47 frames**,
merchants stood still for 34 of them and then entered a tile on every one of frames 35…45.

**The driver was never at fault.** Exactly one `Turn_Tick` runs per frame at every point of
a turn; there is no accumulator, no deferral and no flush anywhere between `winit` and
`advance`, and `tests/pacing.rs`'s `a_frame_of_a_turn_is_exactly_one_turn_tick` passes both
before and after the fix. The deferred-then-flushed theory was ruled out by measurement
rather than by argument.

What was missing is the other arm of `Unit_StepOnce` (`0x0046634D`). It has two, picked by a
latch at `+0x14B` bit 0, and we had only the one that enters a tile:

```c
cVar1 = onRoad ? 0 : 3;
if (cVar1 < ++field_0x14a) {
    field_0x14a = 0;
    field_0x149 += (g_multiplayer == 0) ? 2 : 4;
    if (field_0x149 >= 0x10) { field_0x14b |= 1; field_0x149 = 0; return 2; }
}
return 1;                       /* still crossing: no tile is entered */
```

Sixteen in twos is eight admissions a tile; a road admits every tick and open ground one in
four. **8 ticks a road tile, 32 an open one**, single player. Ours entered one a tick — 6×
and 24× too fast. `[V]`

**The tick rate was never the problem, and it is worth writing down because the obvious fix
was to change it.** The frame loop at `0x004B99C0` runs `Turn_Tick(); Units_Tick();`
`local_c` times, and `local_c` comes from `FUN_004BB978`, `Map_ScrollThrottle`'s twin: one
tick when elapsed >= `((100 - g_optGameSpeed) / 10) * 10 + 2` ms against a `timeGetTime`
stamp, remainder discarded. The shipped default is `g_optGameSpeed = 90` (`0x004AE310`
writes `0x5A`), so **12 ms** against our 16. Ours sits just inside it. Only the
ticks-per-tile were wrong.

### The half of the fix that was missing, and the tests that said so

The branch that found all of the above left **22 tests red** and reported *"8 in
`military.rs`, and nothing else"*. Two separate things had gone wrong, and they are the
reason this entry is here rather than in a commit message.

**One: `cargo test --workspace` fail-fasts.** `long_game.rs` and `turn.rs` sort before
`military.rs` and never ran; the whole of `l2-kingdom` is a crate later and never ran
either. That is `docs/agents.md`'s *how to ablate wrongly*, item four — **read which tests
went red, not how many** — arriving in a handoff rather than in an ablation, where nothing
in the tree can catch it.

**Two, and the substantive one: `Unit_Spawn` (`0x0046E1B0`) ends `field_0x14b |= 1`.**
`Army_Split` (`0x00437FD7`) sets it again on the half it makes. **Every unit the original
creates starts at a tile edge**, so the first admitted tick of its life commits a tile
immediately and only the tiles after it cost the full crossing. `Unit::new` had
`at_tile_edge: false`, which parks every unit in the game for its first eight — or
thirty-two — ticks.

`Unit_Step` (`0x00465D28`) is where that is legible: its `while` loop does the budget test,
the waypoint advance and the tile entry **only** inside the latched arm, so a unit that
stopped for want of moves is standing on a tile edge by construction. That is also why
`l2-scenario`'s default for the three bytes it cannot yet import is the latch **set**.

**Fourteen of the twenty-two red tests were that one line.** All nine in `military.rs` — the
file the handoff said needed fixtures — and five of the driver's own unit tests, every one
of which ticks once and reads the result, which is exactly what the original does. The
`march()` helper the previous agent wrote, and reverted rather than half-apply, was the
right instinct applied to the wrong file: **`castles.rs` genuinely needed it and
`military.rs` never did.**

> **A fix that is half-implemented and a fixture that is staged wrong produce the same
> red.** The difference is visible only from the binary, and the handoff had already
> narrated it as the second.

### The three that were really about the tests, and the one asserting the defect

* **`a_unit_enters_one_tile_a_tick` asserted the defect** — in its name, its doc comment
  (*"fifteen points on a road is fifteen tiles, and it takes fifteen ticks"*) and its
  `assert_eq!`. It is now `a_unit_enters_one_tile_every_eight_ticks_on_a_road` and asserts
  the arrival **ticks** as well as the tiles: 1, 9, 17 … 113. The 8 is typed rather than
  computed from `SUBTILE_SPAN / SUBTILE_STEP_SOLO`, so ablating either constant cannot move
  the probe with it.
* **`the_wait_follows_the_units` was staged wrong** — `for _ in 0..10` against a three-tile
  march that now takes 17 ticks. A bigger fixed number would have been the same mistake
  again; it is a run-until with `assert_eq!(ticks, 17)`.
* **`an_army_ordered_through_the_game_actually_moves_when_the_turn_is_ended` was both**, and
  correcting it turned up a rule nobody had written down. `Units_ResetMoves` (`0x004651B9`)
  is phase 7 and its loop is unconditional — `moving = 0; movesUsed = 0` over all 150 slots
  — so **an unfinished march is dropped at the season boundary, not carried across it.**
  `[V]`. A fifteen-tile road march is 113 ticks and the phases are shorter than that, so
  the army stops part-way and the order is gone. That is not a defect: phase 4 has no clock
  but the turn timer, and the player watches his army walk and *then* presses End Turn.
  **It is the reason `march()`-before-`end_turn` is the correct fixture rather than a
  convenience** — a test that presses End Turn in the same breath as the order is a
  different scenario, not a shortcut.

### And a function that was implemented and never called

`tests/long_game.rs`'s twenty-turn invariant sweep went red on *"realm 5 is allied to 2,
which is out of play"* — on a trajectory the slower world produced and the faster one never
had. `Diplo_ReconcileAlliances` (`0x004A1847`) is what takes such a pairing down, it is
called from `Turn_Tick`'s phase 7 as the last work before `Turn_AdvancePhase`, and
`l2_kingdom::diplomacy::reconcile_alliances` **had no caller anywhere in the workspace.**
`Kingdom::reconcile_alliances` wrapped it and nothing called that either. It is now
`Pass::ReconcileAlliances`, appended to `SEASON_PIPELINE` — appended, so every existing pass
index is unmoved and `l2_game::save`, which writes a pass as its position in that array, is
unaffected.

That is *a producer that is complete and a consumer that is absent* with the missing
consumer being **the game itself**, and nothing in the tree could see it: the function has
unit tests, they pass, and they call it directly.

**The invariant was also stronger than the original.** `Diplo_ReconcileAlliances` `continue`s
on `strength == 0` before it looks at that realm's `ally` byte, so **a dead realm's `ally` is
stale by construction** — realm 2 dies pointing at realm 5, realm 5's own pairing is dropped
on the next pass, and realm 2 goes on naming 5 for ever. And the dead-partner test reads a
`handled` array the same ascending loop is filling, so it can only see indices *below* the
one being walked: an in-play realm allied to a **higher**-numbered dead realm keeps the
alliance. Both are the original's, both are reproduced, and the check now asserts what the
binary maintains rather than what an alliance ought to be.

### What is still open

* **`SUBTILE_STEP_NET`** — the multiplayer `+4` that makes a network game's units walk at
  twice the speed. Named in `tables.rs` rather than dropped. Nothing below `l2-game` knows
  whether the session is networked, and putting `g_multiplayer` into `Kingdom` would put a
  session property into the lockstep digest. Both peers take the same arm, so the value
  agrees where it matters; what is missing is a way to *select* it.
* **`l2-formats` does not read `+0x149 … +0x14B`** out of the original's save, so
  `l2-scenario` defaults them. Bounded and stated at the call site: the original writes its
  save from phase 7, after `Units_ResetMoves`, so nothing in a saved position is walking.
  `docs/agents.md` reserves that crate for the lead session.
* **The late-game evidence was gathered in a faster world than the game has.**
  `long_game.rs` now sees 30 battles instead of 65, no winner instead of a win on turn 212,
  and a first bankruptcy on turn 480 instead of 144 — the mechanism is interception, because
  an army sent at an enemy now spends most of a season walking and the enemy has moved.
  Mutiny and a tax rate >= 20 are reached at no horizon tried up to 1,200 turns; those two
  assertions were **removed, not stretched**, with the measurement written down where they
  were. They need a dealt board, not a longer run.
**C135 — `docs/draws-map.md` names three of the sidebar's industry painters in the wrong order, and the code that reads it would have drawn the wrong icons.**

The campaign-map audit's §2 listing has `FUN_00410502` as *strip row: stone*, `FUN_00410598`
as *wood* and `FUN_0041062E` as *iron*. All three are wrong, and the fourth and fifth
(`FUN_004106C4` weapons, `CountyStrip_DrawCastleIcon`) are right.

Two readings from different directions settle it, and neither is the listing's:

* **`CountyStrip_Draw`'s own dispatch.** `FUN_0040FEC1` fills the right-hand list with
  **labour slots** — 6 wood cutting, 4 iron mining, 5 stone quarrying, 7 blacksmith, 3 castle
  — and the walk is `if (slot == 4) FUN_00410502(); else if (slot == 5) FUN_00410598(); else
  if (slot == 6) FUN_0041062E();`. So `0x00410502` is **iron**, `0x00410598` is **stone** and
  `0x0041062E` is **wood**.
* **`Unit_TrampleTile` (`0x0046873F`).** Its four arms each touch three fields of one
  commodity, and the third is the forecast word the row draws: the iron arm zeroes
  `industry[1].disabledSeasons`, `industry[1].efficiency` and `*(int*)(industry + 2)` — and
  `*(int*)(industry + 2)` is what `FUN_00410502` passes to `Ui_DrawDelta`.

This is the shape `CLAUDE.md`'s table now warns about for `docs/formats/`, arriving in a
document that has no such warning on it: **a listing is an input to the code.** The three
rows were about to be drawn from it, and the visible result would have been an iron icon over
the stone row's number in a county that had all three — a defect no test in this workspace
could see, because nothing here knows what a quarry looks like.

The listing's *counts* are unaffected — 2, 2, 2, 3, 8, seventeen in all — so the audit's
headline number stands and only the three names move.

**C136 — the same tail, one function along: `Industry_LabourEstimate` writes four things and we carried none of them.**

C123 is `Grain_LabourEstimate` (`0x0044D374`): a search loop we ported faithfully and a tail
we stopped before, whose four writes are what the sidebar's grain row draws. **The industry
rows are the identical defect in the identical shape**, and it was found by asking what
`FUN_00410502`'s `Ui_DrawDelta` argument was rather than by looking for it.

`Industry_LabourEstimate` (`0x0044F318`) ends:

```c
iVar3 = Industry_EfficiencyRamp(county, industry, labour[slot].workers, base);
county.industry[industry].efficiency = (char)iVar3;                       /* still not ported */
made = Pct(labour[slot].workers / divisor, local_28);
if (made > limit) made = limit;
county[0x2A8 + industry*0x18] = made;                                     /* now ported */
if (industry == 2) { county.field_0x280 = weaponCost[type].wood * made;   /* not ported */
                     county.field_0x284 = weaponCost[type].iron * made; }
```

**The forecast's offset is the interesting part.** It is not `+0x2A0 + c*0x18`, the record's
own `total`; it is `+0x2A8 + c*0x18` — the **head word of record `c + 1`**, four bytes the
record layout leaves unnamed. So the four values are a second per-commodity array interleaved
with the production records and shifted one whole record along, and stone's lands at `+0x2F0`,
which is inside a 768-byte county and not an overrun. `Unit_TrampleTile` groups the two
offsets in one arm per commodity, which is what makes this `[V]` rather than arithmetic.

**And `+0x290`, wood's own record head, is the weapon type.** `FUN_004106C4` picks the
blacksmith's icon as `county[+0x290] + 0x30` and `Industry_LabourEstimate` indexes
`&g_weaponCost + county[+0x290] * 8` with the same byte. `County::weapon_type` has carried
the comment *"**Engine state.** Which weapon the blacksmith is making"* with no offset since
it was written; it has one.

**Two of the four writes are deliberately still missing, and the efficiency one is the
expensive one.** The estimate pass *mutates* `industry[c].efficiency`, on top of the ramp
`Industry_Produce` applies each season — and `County_RefreshEstimates` runs **four times**
inside `Industry_ToggleFromMap` alone. Porting it changes production numbers on every path in
the game. That is a simulation change needing its own validation and its own fixture, not a
side effect of drawing a sidebar, so it is named at the loop rather than left silent —
`docs/agents.md`: *"port a function's tail with its loop, or say at the loop that you did
not."*

**A third thing fell out of the same function and is not built either.**
`Industry_ProduceAll` writes `industry[c].capacity = labour[slot].workers` after every pass.
`County::industry[].capacity` is documented here as *"written by the industry driver from
county `+0x108`, whose meaning was not traced"* — it is the worker count, and it is the
divisor of the efficiency ramp's overstaffing term. Same class of change, same reason for not
making it now.
**C137 — The first behavioural comparison with the original, and the two
saves it could not have been built on.**

Everything this project checked was a set of **names** (`docs/arms.json` against the `//
arm:` markers) or a block of **static data** (`crates/l2-sim/tests/oracle.rs`, three battle
tables read out of `Lords2.exe`). **Nothing compared behaviour**: no test started the
original's state and ours from the same point, advanced both, and looked at the difference.
`crates/l2-game/tests/differential.rs` is that test — import the before save, run
`l2_game::turn::end_turn`, and compare the result against the after save's own bytes
through `l2_formats::save::{County, Realm, DiploPair, Globals}`.

**The pair it was briefed to use is not a pair.** `battle-before.sav`, `battle-during.sav`
and `battle-after.sav` all read `g_turnCount = 5`, `g_season = 4`, `g_year = 1269`: they are
one battle caught at three moments inside **one** turn, which is exactly what
`crates/l2-game/tests/seam.rs` uses them for. `siege-lastturn` / `siege-sieging` /
`siege-aftersie` are the same, all turn 14. A differential built on the filenames would have
run a season into a kingdom and compared it against *the same turn*, and every number it
produced would have been wrong in a direction nobody could have guessed from the output.

What *is* a turn apart is the autosave rotation — `Save_RotateAndWrite` does
`safeturn.sav <- old_turn.sav <- lastturn.sav` — so a fixture directory copied in one go
holds three consecutive turn openings. There are **four** one-End-Turn pairs on disk and
none of them is the pair the names advertise. `the_pairs_are_one_end_turn_apart` asserts the
gap from `g_turnCount` rather than believing a filename, which is the same lesson as
`l2_testkit::england_turn1`'s fingerprint: **a name is not an identity.**

**And the number the instrument would have flattered us with.** The raw agreement is 877 of
932 fields, 94 %. Ablating the `end_turn` call — comparing the imported before-state against
the after-save with **no turn run into it at all** — still scores **623 of 932, 66 %**,
because most of a county record is inert across a season and an inert field agrees for free.
So the report counts twice, and the second count is the one that means anything: of the 279
comparisons where **the original's own value moved**, we agree on 243 (87 %), and under
ablation that number is **0**. A differential that quoted only the first column would have
reported two thirds agreement for an engine that did nothing whatever.

Its first run found three things, reported rather than fixed: the human realm's `strength`,
`score` and `rank` are never recomputed, because `Realm::sync_score_inputs` is reachable only
through AI step 14 and `l2_kingdom::ai::begin_turn` marks a human realm done before step 0
[V]; neutral counties buy 50 sacks of grain a season that ours cannot, which is
`ai_farm::NoMarket`'s own documented gap finally measured [I]; and `g_optAiLords` is read by
the save reader and dropped by the importer [V].

**C138 — C132's replacement assertion compared two counties
that were both off the edge of the screen, and passed with the outline put back.**

C132 removed the yellow outline round the selected county and wrote the assertion that
could not exist while it was there: `the_selection_is_not_drawn_on_the_map` requires the
whole 480 × 480 map area to be **byte-identical** under two different selections. Its own
entry says why that is the right shape — *"a stronger claim than the outline is gone,
because any future selection paint fails it"* — and it is the right shape. The claim was
still false, for a reason that has nothing to do with the shape.

The test took the **first two** counties the player does not own. On the England fixture
those are 1 and 2. The viewport the map opens on shows counties **8 and 9** and nothing
else. So the two canvases were identical because neither county had a pixel on screen in
either of them, and the assertion was measuring an empty set.

**Measured, not argued: the outline was put back — nineteen lines walking the pick plane and
writing `ink.highlight` on every boundary pixel of `game.selected` — and the whole suite went
green.** 2,158 passing, nothing red, the defect a player had reported four merges earlier
back on the screen. The three defects this branch was sent to check were all genuinely
fixed; the one that was supposed to *keep* the third fixed was not holding anything.

This is `docs/agents.md`'s *"a check that passes for an accidental reason is
indistinguishable from one that passes for the right reason"*, and it is the fourth
instance. What makes it worth its own entry is **where the accident was**: not in the
comparison, which is exact, and not in the threshold, because there is no threshold — it is
in the **selection of what to compare**, one line above, chosen by an ordering that has no
relationship to what the painter can see. C132 deliberately *derived* the two counties
rather than naming them, for a good reason it states, and derived them by the wrong
property.

> **A byte-exact comparison over the wrong two inputs is as vacuous as a threshold chosen by
> observation, and it does not look like it, because nothing in it is approximate.**

The repair has the guard in it rather than beside it. The sweep is now every foreign county
the pick plane says is **on screen**, each against a baseline selection that is off screen,
and an `assert!` that the visible set is non-empty is what stops it going vacuous again.
Ablated: the outline back turns it red with *"county 9 fills 40,286 pixels of the viewport,
and selecting it instead of the off-screen county 1 changed 864 of them."*

**And the general form, which is the part to carry.** Every ablation this project has run
asks *"does deleting the line turn it red?"* The line here was already deleted; the question
that finds this class is the other one — **does the test still pass if you put the defect
back?** For an assertion whose subject is an *absence*, that is the only ablation there is,
and C132 did not have it available, because the outline had been deleted in the same commit
that wrote the test. A test written against a tree where the defect is already gone has
never been observed failing, exactly like a test written against a passing tree.

**C139 — three defects a player reported on the campaign map, and not one
of the arms they lived in was in the arms inventory.**

A player reported three things about selection on the campaign map: an army that could not be
deselected, a yellow outline round the selected county, and a click on grass opening the tax
window. All three were already fixed when this branch went to look — C61 for the first and
third, C132 for the second — and the fixes are real. They were verified by ablation rather
than by reading: removing the deselect turns
`the_right_button_deselects_an_army_and_does_not_open_the_information_panel` red, and putting
a county-selection arm back at the tail of `Map_Click` turns
`a_click_on_a_countys_open_ground_selects_nothing` red.

**What was not there is the bookkeeping.** `docs/arms.json` had **no record for `Map_Click`
(`0x0043CE1A`) at all**, and none for `Screen_FrameInput`'s `0x10` arm. That is 1,263 bytes
holding the whole of what a left click on the campaign map does — the only writer of
`g_screenId = 2` in the binary — plus the four-clause arm that *is* move-order mode, and both
were outside the set rule 5 is measured over. The right-column group covers `x >= 478` and
stops; nothing covered the 480 pixels to the left of it.

So the count CLAUDE.md quotes had been taken over a denominator that excluded the screen the
game opens on. Thirteen records now exist under a new `campaign-map` group — eleven
reproduced, with markers, and two missing — and the reproduced figure moves from 154 of 178
to 165 of 191. **The percentage went down**, which is the correct direction for a change that
only adds things we already do: it was measuring a smaller world.

This is the file's own warning about itself arriving in practice —
*"C61 counted the arms of `Screen_FrameInput`. That is a PLACE, not a category, so every
input the game dispatches from anywhere else scored zero WITHOUT EVER APPEARING AS A MISS"* —
and the new case is sharper than the three already listed there, because `Map_Click` **is**
dispatched from `Screen_FrameInput`. It was not missed for being in the wrong place. It was
missed because the group that owns the campaign map was scoped by a *coordinate* and the
arms are on the other side of it.

Two things fell out of reading the function that are worth having:

* **The gestures differ by one screen id.** `Map_Click` is reached on
  `g_mouseLeftReleased`, so every arm in it is a `left-release`; the confirm one screen away
  on `0x10` is `g_mouseLeftPressed`. Two edges of the same button, and until now neither was
  recorded.
* **`Msg_Enqueue(…, 0x70, …)` fires in exactly two places and not four.** A settlement or a
  merchant in a county that is not yours gets the message; that county's *town* and its
  *farmland* fall out silently, because their owner tests are inside the flag branch rather
  than beside it. We answer both with a status line of ours and enqueue nothing, which is
  filed as `0x0043CE1A/foreign-county-refusal`, missing.
**C140 — the eighteen unaudited call sites, counted: fourteen
of the twenty are drawn by us, five of those fourteen were wrong, and the thing
that was wrong was not the alignment.**

C119 found that `Ui_DrawNumberRight` (`0x004030C6`) **centres**, fixed two panels, and left
`docs/symbols.md` saying *"Twenty call sites in the image inherit it; two are on the county
ration panel and **eighteen are unaudited**."* This is that sweep. Three things came out of
it and only the third is the defect.

**One — the sidebar report that motivated the sweep was already fixed, by C127, and by a
different cause.** *"Happiness # and population # in the sidebar are slightly left of where
they should be"* is `Ui_DrawNumber`'s missing lead column, not this function's anchoring;
`CountyStrip_Draw` (`0x0040F7D3`) draws all three through `Ui_DrawNumber`, which has no
width argument. Re-checked against the binary rather than against C127: the three call sites
are `(pop, ' ', " ", 0x1FC, 0xBD)`, `(happiness, ' ', " ", 0x25A, 0xBD)` and
`(taxRate, ' ', "%", 0x1FA, 0xE2)` — `&DAT_004D3D34`, `…38` and `…3C` hold `" "`, `" "` and
`"%"` — and `screens/county.rs` now passes exactly those. **The sweep is still the job; the
sidebar was one instance of a different thing.**

**Two — the count was wrong in the direction that flatters, and the siblings are the reason
to have looked.** Not eighteen unaudited: `Panel_Ration` holds **five** of the twenty, not
two, and the produce rows, the court and the battle-master ratings had already been moved to
a centring helper. The live tally is **fourteen call sites drawn by us and six not drawn at
all** —

| painter | sites | `x`, `y`, `width` | ours |
|---|---|---|---|
| `FUN_004100AF` / `FUN_0041023A` | 2 | `0x1E0`, row, `0x3C` | `body_number_centred` — **placed** |
| `Panel_Ration` `0x00411B72` | 5 | `0xD0`/`0x10A`/`0x144`, `0x11E`/`0x134`, `0x40` | **two pixels left** — see below |
| `Court_Draw` `0x00416925` | 1 | `i*0x38 + 0x54`, `0x10A`, `0x38` | `Pen::number_centred` — **placed** |
| `Screen_BattleMasterRatings` `0x00421707` | 6 | `c*0x32 + 0xAD`, six rows, `0x3C` | `Pen::number_centred` — **placed** |
| `FUN_00407F82` `0x00407F82` | 1 | under `Flags1a` frame `0x82`, width = **that frame's own width** | **no draw** — the besieger's `unit +0x19C`, siege seasons left, over a besieged castle; `map.rs` draws a dot and says so |
| `FUN_0041A639` `0x0041A639` | 1 | `0x1A8`, `0x1BA`, `0x32` | **no draw** when this was counted — the turn timer. **Corrected by C158:** its guard is `0 < g_optTimeLimit` and `DAT_0055403C < 1 || aiStep == 999`, not `aiStep == 999`, and it is drawn now; `docs/draws-map.md` §5.11 |
| `FUN_0042130F` `0x0042130F` | 2 | `0xA6` and `0x37`, `i*0x1E + 0x6C`, `0x14` | **no draw** — the skirmish army panel; group 11, and we have no skirmish mode |
| `FUN_00423530` `0x00423530` | 2 | `0x1FA` and `0x24A`, `0x1A6`, `0x38` | **no draw** — `g_battleMenA` and `g_battleMenB` on the battle HUD, the two numbers `Battle_CheckOutcome` ends the battle on |

**And the siblings, because the brief was right that a misleading name usually has one.**
`FUN_004025D7` — the centring tail — has three wrappers and twenty direct call sites of its
own. Two of the three wrappers are `Ui_DrawCentred` and `Ui_DrawNumberRight`. The third is
**`FUN_00403190` (`0x00403190`): `Ui_DrawDelta` with a `width`, a signed forecast centred in
a box, `Ui_NumberToBuffer(v, 1, 1)` and the same tail. It has zero call sites.** The one
live sibling, `FUN_00403015` (`0x00403015`), is a **byte-identical duplicate of
`Ui_DrawNumber`** calling a duplicate `Ui_NumberToBuffer` (`FUN_00402447`), six call sites,
no width. So the alignment question closes here: `Ui_DrawNumber` (190 sites; this said 191, counting the definition line, C163), `Ui_DrawCount`
(81), `Ui_DrawDelta` (59), `Ui_DrawHappinessDelta` (2) and `FUN_00403015` (6) take **no
anchoring argument at all**, and the only routine in the image that can be got wrong this
way is the one C119 already named.

**Three — the defect, and it is the same mistake one argument to the left.** Our
`Pen::number_centred` built `" {value} "` for every caller. The suffix is *inside what gets
measured*: `FUN_004025D7` is `x + max(0, (width − FUN_004014F0(buffer)) / 2)` and
`FUN_004014F0` charges four pixels for a space wherever it sits and trims nothing. So an
invented trailing space widens the measure by four and moves the digits **two pixels left**.

Read out of the image, all twenty suffix pointers: **fifteen hold a single space and the
five on `Panel_Ration` hold a NUL** — `&DAT_004D3E04`, `…08`, `…0C`, `…10`, `…14`, five
addresses inside a run of zero bytes in `.data` ending where `"villani1.pl8"` begins. Every
lead is `' '`. Measured on the England fixture: county 8's dairy column draws 505 at x
**343**, and it was drawing it at **341**.

`Panel_Ration`'s *sixth* number has the same suffix and the same mistake in a different
shape. The `Armies Eat` tail is `g_penAdvance = 0; Ui_DrawNumber(men, ' ', "", 0x88, 0x150);
Eng_DrawString(87, 8, g_penAdvance + 0x88, 0x150)`, and `Ui_DrawText` ends with
`g_penAdvance += 4` — which is `shell::TRAILING`, which `Pen::body` already adds. The
`" {men} "` we built charged that gap twice and put *"are foraging"* four pixels right.
**The comment directly above that line transcribed the arguments correctly, `' ', ""`, and
the line below it did not use them** — `docs/agents.md`'s *a correct explanation sitting
directly above the omission it describes*, for the fourth time.

**What generalises.** C119 is *a name is a claim*; this is one level in. Having established
that the function centres, we then handed it a string the original never builds — so the
anchoring was right, the coordinates were right, the width was right, and the picture was
still wrong, because **the argument that decides where a centred string lands is not only
the box, it is also the string**. A sweep prompted by an alignment bug found no remaining
alignment bug and five string bugs. The check that would have caught it is the one this
sweep actually ran: *read every argument at every call site out of the image, including the
ones that look like punctuation.*

---

**C141 — a refactor moved the draw audit by four, and no pixel changed.**

`tools/draws/screendraws.js` counts **one call site, in the source text, of a leaf draw
primitive**, and *"a call inside a loop counts once"*. That rule is stated on both sides and
is the right one for comparing two pieces of source.

It has an asymmetry nobody had named. **The original's source cannot be refactored and ours
can.** C140's sweep replaced five `number_centred` call sites on the ration panel with one
closure called five times — the five draws still happen, at the same coordinates, with the
values now correct — and `draws-ours` fell 418 to 414 while `draws-real` fell 443 to 439.
The original's side did not move, so the ratio got worse because we tidied our code.

Neither direction of the error is small. A painter written as a loop scores below one
written as five lines; a painter written as five lines scores above the same work in a
helper. So **the audit rewards a particular source style on our side only**, and any trend
in the figure across commits mixes real coverage with that.

This is `docs/plan.md`'s own family, third member, and it is about the instrument rather
than the count: C126 says a count can be true and misleading by **weighting**, the
`battlefield.rs` hole says a **denominator can be incomplete**, and this says the **unit can
move on one side only**. All three say: quote the second column, not only the count.

Not fixed, and deliberately. Making the audit count runtime draws would need a harness that
executes both painters, which is a far larger instrument than the one C137 just built and
would answer a different question. Making it count "logical draws" on our side needs a
judgement per call site, which is the hand list the script's header refuses by construction.
What is done instead is this entry plus a line in the script's header, so the next person
who watches the figure fall knows to ask whether anything was lost before treating it as a
regression. **The figure is a coverage estimate with a known style term in it, not a
measurement.**


**C142 — the tax panel's two dead readouts were fixed at the
control and never at the load, and the importer is where a reader should have looked.**

A player, on a build that already carried C125: *"'People pay 0 crowns' on the tax thing
always says 0 crowns. and the happines bonus / minus on tax screen is also stuck and not
adjusting."*

**C125 was right and it was half the fix.** It found that `Tax_IncreaseCounty`
(`0x0043AA83`) is `taxRate++`, `Tax_RecomputePreview`, `Panel_Tax()` and that ours wrote
the rate and returned, and it wired the control up. Measured now: stepping the arrow does
move both numbers, on the bare screen and through the whole `Machine`. What it did not ask
is **what those fields hold on the frame the game is loaded**, and the answer was nothing.

`crates/l2-scenario` read county `+0x0E`, `+0x11`, `+0x12`…`+0x17` and `+0xBC` out of the
county record and **not `+0x0F`, `+0x10` or `+0xC0`** — which are, in order, the tax panel's
*This county* line, the ration panel's health delta, and the tax panel's *People pay*. All
three arrived as `County::new()`'s zero. So a freshly loaded game's tax panel said *"People
pay 0 crowns"* at any rate and drew `( 0 ☺ )` where the original draws `( +5 ☺ )`, until the
player touched a control and C125's recompute filled them in. **That is the player's sentence
exactly**, and it survived a correction written about the same two lines.

> **A control that recomputes is not the same claim as a field that is carried.** C125 asked
> *"who writes this when the player acts?"* and answered it. Nobody asked *"who writes this
> when the game is loaded?"*, and the two questions have different answers and different
> code.

It is the `County::farm_style` shape (C62) and the `Unit::mission` shape a third and fourth
time, and the reason it evaded the exhaustive-destructuring defence is worth stating: that
defence is on `CountyState`, and **a field that was never added to `CountyState` cannot be
caught by a check that enumerates `CountyState`.** The struct literal makes it impossible to
*drop* a field on the way through; it says nothing about a field the reader never read. The
producer is protected and the consumer is protected, and the gap is upstream of both.

**The save is a better oracle than we had recorded, and it settles two open questions.** `[V]`

* **`+0x0F` is `5 - taxRate` in every owned county of every save on this machine** — 5 at
  rate 0, 2 at rate 3, −1 at rate 6, −3 at rate 8 — which is `Tax_RecomputePreview`'s second
  statement and is what says the offset is the right one.
* **`+0xC0` is `Pct(Pct(population, castleBase), taxRate)` to the unit**, in eight of the
  nine counties that carry a rate above zero: `sieging.sav` county 1 stores **147** for 767
  people at rate 6 with no castle; `safeturn.sav` county 2 stores **283** for 738 at rate 8
  behind a wooden castle. This is the **first oracle this project has had for a non-zero tax
  rate**, and it promotes the preview arithmetic from *"our two implementations agree"* to
  *"the original wrote this number."*
* **The ninth is the argument for reading the byte rather than recomputing it on load.**
  `battle-during.sav` stores 245 where the current population gives 225, because the battle
  has already taken the people and the preview still holds the pre-battle answer;
  `battle-after.sav` stores 225. No recompute can produce 245. The original restores a memory
  image and `Tax_RecomputePreview` runs on a control or at the end of a season, never on a
  load.
* **`docs/kingdom.md` §1.3 lists `taxShown` beside `taxCollected` and says it does not know
  how the two differ.** C125 answered it from the code (`Tax_RecomputePreview` has no
  suppression test) and marked it `[D]`. The saves show a second, commoner cause and it is
  not suppression at all: `safeturn.sav` county 2 stores 283 shown against 249 collected,
  and 283 is this season's population while 249 is last season's. **They differ because they
  are computed at different moments**, and the preview is the fresher.

**`docs/plan.md` §2.5 is wrong about the evidence and it is worth correcting, not only
patching.** It says *"every county in every fixture is at rate 0"* and lists the tax
happiness terms among the rules that therefore *"have no oracle at all"*. True of the England
fixture; false of the turn pair and the six siege saves, which carry rates 2, 3, 6 and 8. The
part that is still true is the part that matters for `g_taxHappinessOther`, which is flat
zero below 20 — so the *preview* had an oracle nobody had looked for and the *empire term*
still has none. **An absence claimed about "every fixture" was measured on one of them**,
which is `docs/agents.md`'s *name the branch* on a corpus instead of a call site.

**What the existing test could not see, which is why a second one exists.**
`stepping_the_tax_rate_changes_the_panel_in_the_same_frame` asserts that some pixel outside
the arrows moved and that `tax_shown > 0`. Both survive this defect completely — the load-time
values are never drawn in the frame it compares — and both would survive a painter that drew
a literal `0` on the *People pay* row, because the happiness lines move on their own and the
field it reads is not the one the pixels came from. Confirmed by ablation: with the import
line deleted, that test stays green. The new one reads the number and the words off the
canvas at a real rate and names the call site in its failure message.

**And the words are now pinned as an equality rather than a promise.** C133's rule — a
screen's strings are its specification — reached the tax panel in the same branch that landed
it, but nothing asserted it. `Panel_Tax` fetches group 86 four times (indices 1…4; index 0,
*"Tax in"*, is drawn by nothing in the whole corpus), the install's strings are mixed case and
our fallbacks are upper case, so *"People pay"* present on the canvas and *"PEOPLE PAY"*
absent is the whole of rule 6 for this screen, in two assertions that go red if anyone
reverts it. **Four of group 86's five strings drawn by the original; four by us.**

**Left open, and named rather than quietly fixed.** County `+0x16` (*Other counties*) and
realm `+0x28` are also unimported — and they are **zero in every county of every save on this
machine**, because no save carries a rate of 20 or more and `g_taxHappinessOther` is flat
below that. Importing them would change no pixel and could not be tested, so they are recorded
here and in `docs/oracle-requests.md` instead of written blind. That is the one thing on this
panel a late-game save would still settle.

**C143 — An enumeration whose denominator says *"every X goes through
these N"* is a claim, and this one was one short.**

`docs/audio-triggers.md` opened by explaining why the sound audit was cheap and the input-arm
audit was not: *"the original funnels every sound in the game through **eight** leaf
functions, so the denominator is a grep."* True of eight of them. There are nine.
**`Music_Play` (`0x004263AD`) has nine call sites of its own**, and every one of them is the
front end — `App_WinMain` at start-up, `FUN_00497A34` on every return to the title,
`Screen_DrawConquest` over the interstitial, `Smk_OnFinished` when the intro films end, and
four more over the credits.

It was missed for a reason worth naming, because it is not carelessness and it will recur:
**`Music_Play` is called by two of the eight**, so from the inside it reads as an
implementation detail of `Music_StartCampaign` and `Music_StartBattle` — exactly the thing
the audit correctly excludes. The nine sites that call it *directly* are invisible from that
angle. The same shape had already cost this file once: its first pass counted 121 and missed
`FUN_004262CF`, a 28-byte forwarder with thirteen callers, and the lesson was written down as
*an absence is evidence only in proportion to how hard it was looked for*. Writing it down did
not prevent the second instance, which is `docs/agents.md`'s standing point about documented
process; `node tools/oracle/sounds.js --check` is the version a machine enforces.

**What the omission cost was a player's report going unanswered while the count said
nothing was wrong.** He said *"I don't hear music"*. C116 found the campaign half and fixed
it. The title screen stayed silent, and `audio::scene`'s front-end arm explained why in a
sentence that is entirely correct:

> *"The original plays no music here: `Music_StartCampaign` is reached from the campaign
> coming up, and the title screen's only sound is `setup.wav`."*

Every clause true; `setup.wav` **is** the music, played by `Music_Play(name, 0, 1)` — the same
looping call the campaign picker ends in. And because no row of the inventory covered
`Music_Play`, the count did not *understate* the gap. It could not see it. **A missing row is
worse than a wrong row**, because a wrong row is a lead and a missing one is a clean bill of
health.

**C144 — C133 searched the right condition in the wrong medium.**

A player: *"Also sorely missing: 'All your people are fed by dairy.'"* C133 searched every one
of `L2.eng`'s 317 groups for *dairy*, *fed by* and *all your people*, found only group 62's
**cut** ration screen, and concluded that *"that string does not exist"* — filing the report
under *a memory that fits is not evidence* alongside two genuine misremembering.

**The readout exists.** `Panel_OpenRation` (`0x0043A846`) is four statements and two of them
are sounds:

```c
g_screenId = 0x19; Panel_Ration();
if (rationAchieved == 0)                              Sound_PlayFile("S021_02.wav", 1, 0);
else if (herd && herdEaten == 0 && grainEaten == 0)   Sound_PlayFile("S021_01.wav", 1, 0);
```

`[V]`. The second condition **is** the sentence: a standing herd, and opening the larder took
neither a cow nor a sack. The game says it by **speaking**, on the frame the panel opens, and
`S021_01.wav` ships. Both arms are now fired; `docs/oracle-requests.md` §11 asks somebody to
listen and confirm the words, which is the only part still `[I]`.

**The conclusion C133 drew was right about strings and wrong about the game**, and the reason
it looked exhaustive is the reason it is worth recording: `L2.eng` really is where this game
keeps its words, and `docs/formats/eng.md` §5 makes 317 groups searchable in a minute. That is
a genuinely good instrument, and it made *"I searched all 317 groups"* feel like *"I searched
everywhere"*. **646 of the install's 771 `.wav` files are somebody speaking.** A game with a
narrator keeps part of its interface text in its audio, and a text search over the text files
is exhaustive only over the half that is text.

**It also revises the standing rule about this player's recollections**, which C133 stated as
*three reversed out of three*. It is two out of three. The pattern that survives is the useful
one and it is now unbroken: **he has never been wrong about the behaviour**, and the ration
panel really did have a readout for exactly the condition he named.

**C145 — the audio inventory was prose, and prose is what rots.**

`docs/arms.json` has had `crates/l2-game/tests/arms.rs` behind it since C61: set equality in
both directions between the inventory and the `// arm:` markers in `crates/`.
`docs/audio-triggers.md` had the same job, the same purpose and **no check at all** — a
hand-marked table saying *"we reproduce 24 of 134"*. Both numbers were wrong: the denominator
by the correction above, and the numerator because nobody had compared it with the code since
it was typed.

It now has the same treatment, in two halves that meet in the middle:

* `docs/audio.json` — one record per site, `id` / `addr` / `prim` / `class` / `arg` **generated**
  from the decompilation by `sounds.js --rebuild`, and `status` / `ours` / `note` ours.
* `node tools/oracle/sounds.js --check` — the file against the corpus, both directions, plus
  the four generated fields per row. Runs where the corpus does.
* `crates/l2-game/tests/sfx.rs` — the file against the `// sfx:` markers, both directions.
  Runs everywhere.

**One deliberate difference from `arms.rs`, and it is not a relaxation.** A `// sfx:` marker
may claim several ids. `Msg_PlayVoice` is asked for at sixteen places guarded by sixteen
values of one countdown, and `audio::voice_tick` is that whole ladder in one function;
fourteen markers on one line would be fourteen claims about one line, which is the kind of
tidiness that makes a census lie. What the check forbids is an id claimed **twice** — that is
the property that matters, and it is the one `arms.rs` is really enforcing too.

**And the status vocabulary is the finding, not the count.** Every one of the 143 sites is
`reproduced` (50), `blocked` (55) or `missing` (38), and a `blocked` record is **required by
the test to name the mechanic** in its `note`. That is what turns *"49 bank sites, ✗"* into
something actionable: 25 of them are the battlefield's per-man state machine, 8 are Smacker
playback, 4 are the shared hit-tester. A `blocked` with no note is indistinguishable from a
`missing`, so the assertion on the note is the whole difference between a verdict and a
shrug.

`dead` is asserted **empty**: nothing in the table is unreachable in the shipped game. The
unreachable audio is on the other side of the question — `Ff_win.wav` ships and no call site
names it, which is a file with no trigger rather than a trigger with no path.
**C146 — The differential's first finding, answered: the human's score
was missing a call site and a field, and the field had been documented for weeks.**

C137 reported that the human realm's `score` and `rank` never move — a flat **50**, which is
`score_gold_bracket` alone, against the original's 576, 590, 1333 and 1334 — and marked the
mechanism `[V]` in our tree but **`[I]` for what the original does instead**, because nobody
had found the function. Rule 5 says that is a finding to report, not a licence to invent.
This is the reading.

**There are two call sites, and both rank every realm.**

1. `Turn_BeginPlayersTurn` (`0x0049B6D3`) writes `aiStep = 0` into **every** realm — 999 only
   for a realm at zero strength — and `AI_RunTurnStep`'s (`0x0049A581`) `isHuman` test guards
   the fourteen handlers *and the counter's increment*, **not the initialisation above them**:

   ```c
   if (g_realms[r].strength != 0) {
       if (g_realms[r].aiStep == 0) {
           Realm_RecountStrength(r);      /* 0x0049B42B, Score_RankRealms inside it */
           Realm_UpdateTotals(r);         /* 0x0049D1E0 */
           g_realms[r].offerPending = 0;
           g_realms[r].aiStep = 1;
       }
       if ((g_realms[r].isHuman == 0) && (aiStep < 999)) { ...the fourteen... }
   }
   ```

2. `Turn_Tick` (`0x0049A010`) phase 7 calls `Score_RankRealms()` a second time, after
   `Season_Advance()`. We already had that one, as `finish_tick`'s `game.rank_realms()`.

So C137's diagnosis was **half right and pointed at the wrong line**. It named
`l2_kingdom::ai::begin_turn`'s `AI_STEP_DONE` for a human as the hole. It is not: the
prologue is not a *step*, and `l2_game::turn::begin_phase` already ran `Game::recount_realm`
for every realm including the human, counter or no counter. What it did **not** run is the
prologue's second line — and `Realm_UpdateTotals` is the only thing in `Lords2.exe` that fills
the six score inputs. One missing call, not a missing ladder. `l2_game::turn::step_zero` is
now the prologue in full, and it clears `offer_pending` too: `l2_kingdom::diplomacy` set that
flag and **nothing anywhere cleared it**, so a realm that once courted somebody was refused as
an ally candidate for the rest of the game.

**And then every realm came in exactly 50 short, once per castle.** `score_inputs[5]` is realm
`+0x4C`, which `l2_kingdom::tables::SCORE_INPUT_OFFSETS` identifies as *castles held* — with
the decompiled C for it sitting in the doc comment — and which **nothing in the workspace ever
wrote**. It carries `x50`, more than the other five inputs combined, so the heaviest term of
the score was structurally zero for every realm in every game. That is `docs/agents.md`'s *a
correct explanation sitting directly above the omission it describes*, and the reason it
survived is the shape that entry names: `Realm::sync_score_inputs` deliberately skips slot 5
and said why, and the skip read as *complete* rather than as *a write owed to somebody else*.
Its writer is `Castle_BuildTick` (`0x004508DE`), verified **exhaustively rather than by
reading**: every instruction in the binary whose operand mentions `g_realms + 0x4C` is one of
seven, and they are that function's clear and increment, `Game_SetupRealmsAndCounties`'
initial clear, `Score_RankRealms` three times, and one painter.

**What it moved.** `realm.rank` agrees on all four pairs now and `realm.score` on both battle
pairs exactly; 877 of 932 fields become **885**, and the number that means something — the
fields the original's own turn moved — goes 243 to **247 of 279**.

**What is left, and it is not a scoring defect.** The siege pairs still diverge on
`realm.strength` (9 against 10) and `realm.score` (âˆ’8). Both are one thing, measured: realm
1's 43-man army sits at (46,41) in all three siege saves, untouched across three turns, and
**our** turn has realm 2's 149-man army destroy it every time — `loser_owner: 1,
loser_destroyed: true`. 43 men is `43 / 5 = 8`, which is the whole of the score gap. It is an
AI army-movement or siege divergence and it is left diverging, because a differential that is
tuned is a differential that has stopped measuring.

**The netcode reading, since it changes nothing here.** Ranking *is* a deferred network action
in the original — `NetCmd_WriteRankRealms` (`0x004449A0`) schedules action `0x31`, whose body
`NetAct_RankRealms` (`0x00448422`) is `Score_RankRealms(); for (r = 1; r < 6; r++)
Realm_UpdateTotals(r);`. That is the **same pair of calls** as the step-0 prologue, broadcast
to every peer rather than hung off one realm's ladder, and it is also the ratings screen's
refresh path. It confirms the pairing rather than contradicting the turn ordering: the two
always travel together, and the one we were missing is the one that is never named on its own.



---

**C147 — a correction went missing because its em-dash was encoded twice, and no
check in the tree could see it.**

An agent wrote a correction heading and what landed in `docs/decisions.md` was
`c3 a2 e2 82 ac e2 80 9d` where the em-dash belonged: UTF-8 read as CP1252 and
encoded to UTF-8 a second time. Every other heading in the file carries the plain
`e2 80 94`. `corrections.js` could not parse the heading, so the correction did
not exist as far as the log was concerned and three citations pointed at nothing.

**The near miss is the finding.** It was caught only because those bytes happened
to sit in a line that a generator parses. One line lower, in the prose, it would
have shipped and stayed — double-encoded UTF-8 is *still valid UTF-8*, so nothing
rejects it, nothing fails to read it, and it is visible only where somebody
happens to look at that paragraph.

**And the check that sounds like the one for this is not.**
`crates/l2-testkit/tests/encoding.rs` asserts that every field of an encodable
struct survives a round trip through `Canonical` — the *simulation's* bytes, not
a document's. A reader who went looking for a text-encoding check would have
found that name and stopped. **A test whose name reads like the check you want is
worse than no test, because a reader stops looking.** That is C119's shape in a
different place: there, a function called `Ui_DrawNumberRight` centres, and the
wrong name was believed twice because a `[V]` comment agreed with it.

`crates/l2-testkit/tests/mojibake.rs` is the check, built as the sibling of
`conflict_markers.rs` and sharing its `TEXT` list deliberately — two
text-hygiene checks disagreeing about which files are text is a gap shaped
exactly like this one. It scans tracked files as bytes viewed as latin-1, so a
mangled run is matched as the byte run it is rather than as whatever it decodes
to.

**Its second test exists because of C138, four merges earlier.** A scanner that
silently visited nothing reports a clean tree, and a clean tree is what this
check reports when all is well — the same failure as the county-outline
assertion that compared two off-screen counties and therefore could not fail. So
the scan carries its file count out, and the test asserts both that it looked at
something and that the matcher matches, on bytes built the way a file's are.

**The first draft of that second test was wrong, and it caught its own author.**
It asserted the patterns were present in the file that defines them. They are
written as `\u{..}` escapes precisely so that the file *cannot* contain them as
bytes and cannot be corrupted into agreeing with the corruption — so the
assertion failed, correctly, on its first run.

Ablated: restoring the mangled heading fails with `docs/decisions.md:6781 em dash
should be '—'`, and removing it passes again.

**C148 — `docs/arms.json` marked nineteen arms `reproduced` under a kind
none of them had, and the thing that caught it was a field written for another purpose.**

C131 added a `gesture` field to `docs/arms.json` and a second word to the `// arm:` marker,
because an arm could be marked `reproduced`, be genuinely present, and be answered with the
wrong *kind* of gesture in every case. It also built the mechanism — `press::Press`, the
48-byte ramp, the twenty-frame countdown — and wired it on one screen.

**What it did not do was change any other screen's behaviour, and the check could not tell.**
`arms.rs` compares the marker's word with the record's word. Both are edited by one person in
one sitting, which is the shape `docs/agents.md` names as *"two artefacts that must agree and
are maintained by the same person, at the same time, for the same reason"* — the pattern that
lies. So nineteen arms acquired the word `left-press-repeat` or `left-press-delayed`, the code
under them went on firing once on the press, and every test in the tree passed. `supplies.rs`
said it out loud in a constant's doc comment — *"**Kind 5**, so the original fires them twenty
frames after the press; we fire at once and record the difference"* — which is the
*correct-explanation-above-the-omission* shape from three other places in `docs/agents.md`.

**Three defects a player reported are this one gap, and a fourth was already fixed.**

| the report | the kind | where it was |
|---|---|---|
| *"clicking yes/no is instant, whereas the game waited on mouse-up, and the gauntlet would go down slightly"* | `Widget_Test` **5** | `g_confirmWidgets`, and there was **no record and no marker for it at all** |
| *"holding on a button doesn't make it go up faster"* | `Widget_Test` **4** | `0x004BA9C8/tax-and-ration-arrows`, filed `left-press` |
| *"the original has the army steps on hover"* | `hover` | built by C61 and **in no inventory**, so this one did not still reproduce |

### The finding: the address that would have answered it was already in the record

`0x004BA9C8/tax-and-ration-arrows` has `addr` = `Screen_HandleInput`, a 3,832-byte dispatcher
that is nobody's handler — so the exe-gated kind-byte check skipped it. And its `what` reads:

> *"`g_taxWidgets` (`0x004DD790`) and `g_rationWidgets` (`0x004DD7C0`), two 24-byte records
> each, up then down"*

**Both of those are kind 4.** The record named the tables, in prose, months before the gesture
field existed, written by somebody not thinking about kinds — and the check that was supposed
to read the kind byte was looking at the wrong field of the same record. Extending it to scan
`what`/`note` for addresses that land on a **record base** inside the two table regions found
four wrong gestures, three of them marked `reproduced`, and ablating the fix makes it name the
player's bug in its own failure message.

That generalises past this file: **before adding a second artefact to check against, look for
one that is already there and was written for another purpose.** A cross-reference in prose is
independent of the field you are checking in exactly the way a freshly written duplicate is not.

**It also took three wrong versions to get right, and each wrong one was a false positive
rather than a miss** — which is the safe direction and worth recording as the reason to build
it this way round. Reading the `id`'s address said kind 5 about `supplies-sheep-row`, whose id
is a **table base** whose record 0 is a thumb. Reading every line said kind 5 about
`enter-confirms`, a keyboard arm, because the `groups` prose at the top of the file names
`g_splitWidgets`. Reading the line that opens a record attached the *next* record's id to the
*previous* record. Each was a wrong answer that argued for itself, and each was found by
opening the row rather than by reading the count — `docs/agents.md`, *read the first three
findings of every new tool's first run*.

### Three smaller things worth keeping

* **A family resemblance is not a kind.** The battle prompt's thumb-up and thumb-down are the
  *same two pictures* as the yes/no box's and a *different kind* — `Battle_PromptAnswered`'s
  records are 4, `Ui_ConfirmClicked`'s are 5. Ours had been filed `left-press`, moved to
  `left-release` by the branch that found `Ui_OkButtonClicked` waits for the release, and is
  neither.
* **A two-sided check has a blind spot exactly between its sides.** `arms.rs` asserts every
  record has a marker and every marker has a record. `Map_HoverUnitTarget` had **neither**, so
  the arm a player reported, which C61 then built, was invisible to both directions of the
  check written to count it.
* **`docs/input.md` said kind 2 was *"one pair, a setup-page scroll"*. It is 23 records over
  seven handlers** — a true statement about one table promoted to a statement about a kind,
  which is the `[V]` failure `CLAUDE.md` warns about, in the document that was itself the
  remedy for the last one. The cure is `tools/oracle/kinds.js`: the counts in that table are
  now generated from the exe, and the script refuses to print anything until two spot checks
  against handlers named from call sites pass.

**What this branch did not do**, said at the loop rather than left silent: our screens declare
a kind for about forty of the original's 332 input records. All five kinds are built and
`Kind::Held` reaches nothing, because all seven of its handlers are skirmish and multiplayer
setup pages this engine does not have.
**C149 — The seam's stated reason was false, and checking it cost fifty
sacks a county a season.**

C137's second finding, closed. `crates/l2-game/tests/differential.rs` measured that the
original's unowned counties 1 and 3 go 71 → 121 and 57 → 103 across `old_turn.sav →
battle-before.sav` where ours went 71 → 71 and 57 → 53 — **50 sacks short, each, every
season**. Phase 1 ran `Kingdom::run_neutral_farms` with `l2_kingdom::ai_farm::NoMarket`,
and `NoMarket` carried the reason at the type:

> *"there is no stall yet, so every style's opening shopping cascade is refused and the
> county farms what it already has."*

**The stall was there. The money was there. Nobody had opened the file.**

`Ai_BuyGood` (`0x004A4B12`) is three gates, and the fixtures answer all three:

```c
if (g_counties[county].merchantCount != '\0') {          /* +0x1A4 — the stall  */
    price = base + max(1, Pct(base, g_units[+0x1A5].morale));
    if (((realm != 0) || (price * qty <= g_counties[county].purse)) &&   /* +0x1F4 */
        ((realm == 0) || (price * qty <= g_realms[realm].gold)))
        Merchant_Trade(0, qty, good, price, base, realm, county);
}
```

`+0x1A4` is non-zero on every county holding a merchant, in all six one-turn-apart saves.
`+0x1F4` reads **186, 297, 260** on county 1 and **195, 316, 294** on county 3 across three
consecutive turns of the battle game, and 436 on siege county 3.

**Where the money comes from, and the sentence in `docs/symbols.md` that hid it.**
`Tax_CollectAll` (`0x0044B59B`) ends in a two-limb branch:

```c
if (realm == 0) g_counties[c].purse += g_counties[c].taxCollected;
else            { g_realms[realm].gold += take; realm.f0xF4 += take; realm.f0xF8 += take; }
```

`docs/symbols.md` describes that as *"credited to the owner realm's gold"* and stops, so the
`realm == 0` limb was missing from the file this project consults to decide what the binary
does. `Kingdom::tax_collect` dropped the take on the floor for months.

**And here is the uncomfortable part: `docs/symbols.md` was the only place it was missing.**
`docs/kingdom.md` §4.1 has carried `if (owner == 0) county.f1F4 += take;` **in its
pseudocode**, with a `[V]` paragraph underneath saying *"an unowned county banks its own tax
into `+0x1F4` rather than into any treasury, which is why the neutral tax ladder in §8.2
exists at all."* `l2_kingdom::ai`'s comment on that ladder says it too — *"nobody is
collecting it, though: `Tax_CollectAll` banks an unowned county's take into the county
itself"*. `County::purse`'s doc says it a third time. **Three documents were right and no
code did it.** That is `docs/agents.md`'s *a correct explanation sitting directly above the
omission it describes*, and it is the first instance where the explanation was in three
places at once — so *write it down* was not the missing step, and adding a fourth sentence
would not have helped. The same is true of `County_RecountMerchants`, which is in
`docs/kingdom.md`'s printed season-pass list and was in no pipeline.
**`+0xF4` and `+0xF8` are a third accumulator pair, not `Realm::trade_received_a`/`_b`,
which are `+0x10C`/`+0x110`; they are named and not ported.**

**The fifty is a computation, and the control is in the same fixture.** Grain's base price
is 2 (`g_goodsPrice`) and every merchant the shipped game creates has morale 100, so a sack
costs `2 + Pct(2, 100)` = **4 crowns**. The neutral grazing cascade offers 400, 200, 100 and
50 sacks in that order, whole lot or nothing, and takes the first the purse covers:

| save | county 1 purse | county 3 purse | 50-sack bill | bought |
|---|---:|---:|---:|---|
| `safeturn` → `old_turn` | 186 | 195 | 200 | **nothing** |
| `old_turn` → `battle-before` | 297 | 316 | 200 | **50 sacks each** |

Purse − 200 + that turn's tax lands on the after-save's purse **exactly**, both counties:
`297 − 200 + 163 = 260` and `316 − 200 + 178 = 294`. So the number is
`max(lot : 4 * lot <= purse)` over the cascade, and a constant of 50 would have been wrong
one turn earlier in the very same fixture. That is the difference between a fix that
generalises and one that fits.

**And it was checked somewhere else, because two fixtures agreeing about 50 would not have
settled it.** Sixty turns driven from `england-turn1.sav` — a different map, a different
game, counties this work never looked at — produce eight neutral purchases, and they are
**not all fifty**: county 3 takes the 50-sack lot out of 297 crowns, county 10 takes 100 out
of 497 (with the arable style's `+100` top-up in front of it), and county 7 takes the top
**400-sack** lot out of 2,003. Three different rungs of the same ladder, each the largest
its purse covers. A constant would have produced 50 in all three.

They are also *rare* — eight purchases in sixty turns, against 178 county-turns where a
neutral county had a stall at all — because the cascade needs `grain < 100` **and** a
merchant standing in the county on that turn. A neutral county's purse therefore climbs a
long way between visits; the sixty-turn run tops out at 22,535 crowns. That is the
original's behaviour and not a leak: nothing in the binary ever spends a county purse except
`Merchant_Trade`, and `County::purse`'s only other writer is the style-0 top-up.

**What it took to build, all of it in the binary's own order.**
`County_RecountMerchants` (`0x00451061`), season pass 22, which was not in the pipeline at
all — it writes the stall; `Tax_CollectAll`'s realm-0 limb; the four county bytes carried
through `l2-scenario` (a save's stall must be *imported*, because phase 1 runs before the
recount does); and `CountyStall`, which is `Ai_BuyGood`. The neutral arable style's
`purse += 100` was already written down in `neutral_purse_top_up` and **reported rather than
applied**, with a comment deferring it to a caller that did not exist — `lay_out` applies it
now.

**And the defect underneath, which is the part worth carrying.** Wiring the cascade made
county 3 end eight sacks *low*. `crate::trade`'s port of `Merchant_Trade`'s tail called
`ration::apply` twice, and ours debits the store — where `Ration_Apply` (`0x0044DF5F`) has
**no store subtraction anywhere in it**, on either of its two passes over the ladder. Every
trade at the merchant made the county eat two extra meals on the spot. It had survived
because its only caller was a person clicking a stall, which no test drives against a
fixture; the neutral cascade gave it a caller the differential watches and it showed up the
same hour, as two helpings of four sacks.

That defect had also written itself into the record. `docs/bugs.md` **B11a** explained that
the unowned county's negative store *"is then erased by the very next statement… the bug is
worth free crowns rather than a visible negative number, which is presumably why it has
never been reported"* — careful, mechanical, and **reasoning about a defect of ours as
though it were a rule of the game's**. A test asserted the erasure. Both are corrected; the
store really is left negative.

> **A stated reason that nothing checks is the same failure whether it appears on a screen
> or at a seam.** `NOT SIMULATED` (C120), the font comment (C107) and `INDUSTRY / NOT DRAWN`
> (C136) were all prose that read as a decision and was never a measurement. This one cost
> fifty sacks a county a season for as long as it stood, and the check was one `grep` of a
> save file at `+0x1F4`.

Result: `MOVED_AGREE_TOTAL` 243 → **247** of 279, `AGREE_TOTAL` 877 → **881** of 932, and
both `county.grain` and `county.grain_available` divergences gone on both counties. The
ablation is stated at each test and was run: reverting phase 1 to `NoMarket` puts the
baseline back to exactly its four old rows and 243.
---

**C150 — A list index and a plane-3 number that both read `2`
were two and one, and the mercenary was painted on the one tile of the town the
original's overlay pass never visits.**

A player: *"I haven't seen any mercenary icons on the town square yet."* The
feature was **not missing.** `crates/l2-view/src/campaign.rs` had
`MERCENARY_MARKER_FRAME = 0x81` and `screens/map.rs`'s `draw_flags` blitted it,
gated on `county.mercenary_offer`, with a comment that named the right quadrant:

```rust
// `county.mercenaryOffer != 0` puts frame 0x81 on the north-east
// quadrant — a standing band, advertised on the map.
if let Some(&ne) = town.get(1) {
```

**Everything in that comment is true and the line under it is wrong.** The
quadrant `Sprite_TopIt` (`0x004071A0`) tests is `tile.part & 0xf`, plane 3,
which `l2-formats` documents as `dx + W * dy` from the block's north-west corner
— `[V]`, 10,971/10,971. For a 2 × 2 town that makes quadrant **2** the tile at
`(x, y + 1)`, which is the **third** tile in index order. `town()` returns tiles
in index order. `town.get(1)` is quadrant **1**, `(x + 1, y)`.

The offset was wrong in the same line. The two town arms are the same shape and
set different offsets, and only one had been read:

```c
part == 0:  zoom 0 (+0x1A, -0x1C)   zoom 2 (+6, -0x15)   local_c = 2
part == 2:  zoom 0 (+0x10, -0x12)   zoom 2 (+6, -0x15)   local_c = 0
```

so the marker went through `Zoom::flag_at` and landed ten pixels right and ten
pixels up of where it belongs. `Zoom::mercenary_at` exists so that the two
cannot be confused again, and `campaign::draw_mercenary_marker` is its own entry
point for the same reason.

**The third source is the one that settles it, and it never reads `part` at
all.** `County_FindTownTile` (`0x00467FD1`) sweeps the grid in index order,
counts the county's `flags & 0x40` tiles, and sets **bank bit `0x80` on the 0th
and the 2nd**. `FUN_00405EB5` is `if (tile.bank & 0x80) Sprite_TopIt(...)`, so
bank `0x80` is the only gate on the pass running at all: the original does not
*visit* the tile we were painting. Two of the town's four quadrants are silent
and we had chosen one of them.

Three things worth carrying:

* **A number that indexes a list and a number that names a position are the same
  integer with different arithmetic behind them**, and nothing in the type system
  tells them apart. `MapScreen::town_quadrant(ctx, county, part)` now computes
  the tile from the `dx + W * dy` rule, so the *quadrant* is the argument and the
  list index never appears.
* **The comment was right.** This is the shape `docs/agents.md` records under
  *a correct explanation sitting directly above the omission it describes* — the
  prose named the north-east quadrant, a reviewer would nod at it, and the code
  under it did something else. What caught it was not reading the comment harder;
  it was sweeping all 44 shipped maps and asking the bytes.
* **Absence of a report is not absence of the feature.** The handoff on
  `armoury-walker-village-merc` had already established, by reading
  `Village_Draw` and `Village_Animate` in full, that **there is no mercenary in
  the village at all** — the "town square" the player means is the county town on
  the campaign map. A brief written from the player's words alone would have had
  somebody adding a figure to `screens/village.rs`, where the original has none.

**And the words, which were the other half.** `CLAUDE.md` rule 6: the map's
marker is a picture with no text on it, and the only place in `Lords2.exe` that
says what it *is* is `TileInfo_Draw`'s tail —

```c
if ((g_pickedTileFlags & 0x80) == 0) {
  if ((g_pickedTileFlags & 0x40) != 0 && county.mercenaryOffer != 0) {
    Pl8_DrawFrameClipped(g_flagsSheet, 0x81, 0x32, R * 0x10 + 0x9c);
    FUN_0040328e(0x1e, 0x3b, 0x68, R * 0x10 + 0xa0, 0x140, ...);
  }
}
```

`L2.eng` 30/59 is *"Mercenaries are available for hire in the county."* and
`TileInfo_Draw` is its **only** consumer. The tile half of `screens/info.rs` drew
none of the group-30 ladder; it now draws the county-town arm — 30/7 *"County
town."*, 30/27, `Icon_tmp.pl8` frame `0x1B` — and that tail. A player who has now
*seen* the figure has somewhere to go and read what it means.

Two smaller corrections fell out of reading the arm:

* **`BODY_WRAP` was one number where the binary has two.** `UnitPanel_Draw`'s
  five wrapped draws pass `0x120` and `TileInfo_Draw`'s three pass `0x130`. The
  constant had been read off a group-31 call site, documented with that call site,
  and used as though it belonged to both halves. `TILE_BODY_WRAP` is the tile
  half's.
* **The tile half's headings are `&g_fontHeading`.** The unit half's use
  `Pen::eng`, which is the *body* font, at the same slot. Not fixed here — five
  call sites in a half this change does not touch — but recorded, because it is
  the kind of thing that reads as a font choice rather than as a divergence.

---

**C151 — three sidebar reports, all three already fixed, and the two that
had no test between them.**

Three player reports were sent together because they are one shape — a `Ui_DrawDelta` on a
produce row, which is the signed forecast for the season about to begin:

> *"Sidebar doesn't show grain being planted as a negative number."*
> *"The figure is missing in the sidebar — it draws the serf reclaiming, but not the +1."*
> *"I right now have −11 cattle. If I move it so the people are eating cattle, it still says
> −11."*

All three were the **tail** of an estimate pass whose search loop had been ported without it,
and all three had been repaired — C123 (grain, `Grain_LabourEstimate` `0x0044D374`), C128
(cattle, `Herd_LabourEstimate` `0x0044DD4D`) and C129 (reclamation, `Field_ReclaimEstimate`
`0x0044C278`). Verified on `main`: painting four fields and reading the sidebar at the sowing
turn draws `-20 ` in `0xF9` on the produce plate, and the herd forecast moves when the ration
split moves.

**So the finding is not the fix. It is what the fix left behind, and there are three things.**

**One: the comments that said the work had not been done outlived the work by a day, in the
function that does it.** `county::draw_produce_rows` carried, ten lines above a `match` that
reads all three fields, *"Only the cattle row draws one … neither is computed anywhere in this
workspace, so neither row can draw one yet."* Its doc comment carried *"this workspace never
computes it"*, and `screens.rs`'s cattle test carried *"nothing in this workspace computes
it"*. Every one was true when written and every one is an instruction to the next reader not
to look. This is *a document that promises "until X" keeps promising it long after X* with the
tense removed — no *"until"*, no *"for now"*, just a statement of fact that expired — and it is
the third time on this project that prose survived the condition it described.

> **A comment that says a thing is not done is a claim with an expiry date, and the commit
> that does the thing is the only moment anybody will ever be in a position to strike it.**

**Two: the report that started it was the one with no test.** Cattle got
`the_cattle_forecast_follows_the_labour_it_depends_on`; reclamation got
`the_reclamation_forecast_counts_fields_finished_not_work_done`; **grain got none at all**, at
either layer. `land::grain_preview` had zero callers in any test in the workspace. Three
repairs in one evening, described in the branch as *"one fix repeated, not three
investigations"* — and the repetition is exactly what let the middle one ship untested, because
the two neighbours reading as covered is what covered reads like. The new tests are
`the_grain_forecast_is_the_sowing_loss_the_player_reported` (the four seasonal arms) and
`the_grain_row_draws_its_sowing_loss_from_the_brush_to_the_pixel` (the road, brush to glyph).

The second is worth the extra cost for a measurable reason: **its two ablations fail at
different assertions.** Delete `strip_delta` and it fails at the glyph search; delete
`grain_preview` and it fails one line earlier, at the simulation's own `shown < 0`. A test that
set the field by hand — which is what the cattle test does, correctly, for a test about
`Ui_DrawDelta` — cannot tell those two apart, and the defect being repaired was the second one.

**Three: the cattle question was never only about the sidebar, and the second half is
answered.** *"I had lots of milk maids with low herd crowding and we were only getting 1 cow,
and if I added more milk maids they were idle."* Both halves are `Herd_LabourEstimate`'s search
loop, which assigns the **fewest** workers that reach the best `births − deaths` — a strict `<`
on the running best. `land::herd_labour_estimate` documented the closed form as *"about
`6 * herd`"* and marked it `[I]`. Measured over every herd size 1 … 400 in all four seasons:

* **six a head is a true bound** and now a `[V]` assertion, because that is where
  `PctOf(labour, herd * 3)` hits its 200 % cap and births stop rising;
* **it is a bad estimate of where the answer lands.** `births = herd * birthRate / 10000`
  truncates, so on a small herd the integer stops moving long before 200 % and the first argmax
  wins: a herd of five tops out at **15** milkmaids — three a head, not six — and a herd of one
  at **1**. That is the player's county, and the blue idle ring is that ceiling being hit.

And *"only getting 1 cow"* is the same truncation from the other end: the rate is applied to
the **herd**, not to the pasture, so lowering crowding raises the percentage and not the count.

**A quirk fell out of measuring it, and it is the original's.** The small-herd birth bonus is a
step — `+10000` below 5 head, `+5000` below 10, `+2000` below 25 — and at every one of the three
boundaries the step down is worth more than the animal that crosses it. Fully staffed at low
crowding in spring: **4 cows → 7 calves, 5 → 4; 9 → 10, 10 → 6; 24 → 16, 25 → 10.** Reproduced
and pinned (`docs/bugs.md` B98); a taper would be ours.

**One repair that is not a comment.** `field::refresh_estimates` carried
`// 's tail — the third of the evening` and `//  C129.` — two backticked spans had been eaten
out of the comment before it was ever committed, leaving a sentence with no subject and a
citation with no document. The citation lint could not see it, because a `C129` with nothing in
front of it still resolves.

---

**C152 — The menu bar's shield row is a turn clock, and
we drew half its guard.**

A player: *"I think in the original game the shield icons at the top meant that
players hadn't ended their turn."* He was right, and his recollections of *what the
game did* remain unreversed.

`Screen_DrawMenuBar` (`0x00419C78`) draws one `Misc_cty` banner per realm under a
guard with **two** clauses:

```c
if ((g_realms[i].strength != 0) && (g_realms[i].aiStep < 999)) {
    Pl8_DrawFrame(g_miscCtySheet, g_realms[i].shieldIndex + 0x55, slot * 0x10 + 0x10e, 4);
    slot++;                          /* only when drawn: the row compacts left */
}
```

`draw_menu_bar` in `crates/l2-game/src/screens/map.rs` tests `Realm::in_play`, which
*is* `strength != 0`, and nothing else. So no shield ever vanishes.

**`aiStep == 999` is "this realm's turn is over"**, from four sites: `Turn_End`
(`0x0043AC23`) writes it the instant the button is clicked; `Turn_BeginPlayersTurn`
(`0x0049B6D3`) clears every realm to 0 at the top of phase 4; `Turn_AllRealmsDone`
(`0x0049B762`) is phase 4's exit condition; and `FUN_004479E9` writes it for a
**remote** seat. The detail that settles intent rather than inferring it: `Turn_End`
also raises `DAT_0056D6A0`, and that flag is half of `Screen_DrawMenuBar`'s own
repaint guard — **the only reason the button touches it is to make the shield go.**

**What is worth carrying is that this gap was already written down, in the file, in
the right place, and it did not cause the work to happen.** `map.rs` carried, twenty
lines from the loop: *"Two other things read the same flag and we reproduce neither
… `Screen_DrawMenuBar`'s banner loop is `strength != 0 && aiStep < 999`, so each
realm's banner vanishes from the menu bar as that realm finishes its turn."* That is
a third instance of `docs/agents.md`'s *a correct explanation sitting directly above
the omission it describes* — and the first where the prose names the **guard clause**
it then does not write. The two siblings are `Screen_DrawEndTurn`'s missing caption
(which we do reproduce) and `FUN_0041A639`'s turn timer (which we do not).

**And the fix is not the literal clause**, which is the part a careful reader gets
wrong. Our turn model inverts the original's: its phase 4 *is* the interactive phase,
with the human parked at `aiStep == 1` while the AI steps behind him; ours parks the
player on the map with the machine at phase 1 and runs 1→7 inside one End Turn press,
so between turns **every** realm's counter is already ≥ 999. A literal `ai_step < 999`
draws no shields at all while the player is playing — the defect inverted. The exact
mapping is the one `map.rs` already found for the End Turn caption:
`turn::turn_in_flight` **is** the human's `aiStep >= 999`. `Realm::turn_done()` is the
wrong instrument here for a different reason — it short-circuits on `is_human`.

**What was built.** `l2_game::turn::realm_turn_ended` carries that mapping —
between turns nobody has ended; inside one the person has, and each AI realm has
when its own `ai_step` says so — and `draw_menu_bar` skips a realm on it. The slot
now advances on every banner the loop *draws*, as `local_c` does, rather than on
every frame that happened to load. The test that holds it
(`screens.rs::the_menu_bar_shields_are_the_realms_still_to_move`) opens on the
trap: every living realm's shield **must** be up during the person's own turn with
every counter at or past 999, so the literal port goes red on its first assertion.
Its expected pixels are built from `Screen_DrawMenuBar`'s own literals — frame
`0x55 + shield`, `x = 0x10E + 0x10 * slot` — and never through the function under
test. `FUN_0041A639`'s turn timer, the third reader of the flag, is
C158.

---

**C153 — the `Industry` record base is county `+0x294`, and four
bytes of Ghidra's guess invented an off-by-one that four places repeated.**

`docs/draws-map.md` §5.5 says the four sidebar industry forecasts each read
*"the record above the commodity its row is for"*, and that the wood row's
`0x2F0` is *"one whole record past the end of a four-record array"* — either an
off-by-one in the original or a wrong base in `docs/records.json`, unsettled, and
named there as a prerequisite that is *"its own job"*. It is the second, the row
in question is stone rather than wood, and there is no off-by-one anywhere.

**The base is `+0x294`, stride `0x18`, four records closing exactly on `+0x2F4`.**
`docs/records.json` says `+0x290` and is four bytes low. Its *absolute* offsets are
all correct, because its field offsets are compensatingly `+4` — which is why the
error never fired anywhere but here, where a fifth field exists that the short base
cannot hold.

Three readings settle it and none is a document's:

* **The span.** Every raw county address in the decompilation used with a `*0x18`
  stride is one of ten, `0x53fc44 … 0x53fc58`; against `g_counties = 0x0053F9B0`
  those are county `+0x294 … +0x2A8`, and the last is four bytes wide. `0x2AC −
  0x294 = 0x18` — **the observed field set spans exactly one stride, with no slack
  at either end.** A base of `0x290` cannot hold `+0x2A8 + c*0x18` at all. This is
  a self-verifying invariant in the instruction stream and needs no save to run.
* **The array's end.** `levySurcharge` is at `+0x2F4`. Base `0x294` ends the array
  exactly there; base `0x290` ends it at `0x2F0` and leaves a four-byte unnamed hole
  which is **precisely the word the stone row reads**. The hole was the evidence for
  the overrun, and the hole was the wrong base's own footprint.
* **`records.json`'s own comment refutes itself.** It reads *"The 0x18 stride is
  confirmed by the byte quad at `+0x04..+0x07` repeating at county `0x294`,
  `0x2AC`, `0x2C4` and `0x2DC`."* Those four addresses **are the four record
  bases.** The measurement was right and was written down against the wrong base.

The two ends agree with no residue. **Painter side:** the four `Ui_DrawDelta`
operands are county `0x2A8`, `0x2C0`, `0x2D8`, `0x2F0` — an exact arithmetic
progression of `0x18`, one per commodity, each `record[c] + 0x14`, drawn by
`FUN_0041062E` wood, `FUN_00410502` iron, `FUN_004106C4` weapons, `FUN_00410598`
stone. **Producer side:** `Industry_LabourEstimate`'s tail writes
`*(int *)(county * 0x300 + 0x53fc58 + industry * 0x18)` — the same expression — and
`County_RefreshEstimates` passes the commodity index. So every row reads **its own
commodity's own record**, and `Industry::next_season` is the record's last field at
`+0x14` rather than, as C136 put it, *"the head word of record `c + 1`"*. C136's
arithmetic was right; its sentence about the layout was not.

**County `+0x290` is not in the array.** It is the standalone `weapon_type` byte —
C136 established that from `&g_weaponCost + county[+0x290]*8` and `FUN_004106C4`'s
frame — and Ghidra swallowed it into `industry[0]`. That one absorbed byte is the
entire origin of the "record above" story, which then propagated into
`docs/records.json`, `docs/draws-map.md` §5.5,
`l2_kingdom::county::Industry::next_season`'s doc table and
`l2_game::screens::county`'s header, none of which had looked at it again.

This is `CLAUDE.md`'s `[V]`-is-a-claim warning arriving for the **third time this
week in `docs/draws-map.md`** — after C135 (three painters named in the wrong order)
and §5.13 (a row right and stale). C124's lesson was aimed at `docs/formats/`; the
campaign-map audit has now earned the same warning, and the pattern is that a
document assembled from decompiler output inherits the decompiler's struct guesses
without ever saying that is what they are. **A Ghidra field name is not evidence.
The instruction's operand is.**

**A fourth reading is the original's own saves, and it is the one that runs.**
`crates/l2-scenario/tests/import.rs` computes, for every county of every save on
this machine, the number `Industry_LabourEstimate` writes from **record `c`'s**
guards and workers, and requires county `+0x2A8 + c*0x18` to hold exactly it:
**1,152 forecasts, 81 non-zero, and 343 where record `c + 1` would have given a
different number** — siege-lastturn county 4 stores 74, which is 93 woodcutters at
80%, not iron's 92 at 80% = 73. Stone is switched off in every save, so its word is
only checked at zero; that is the corpus's limit, not the reading's. Putting the
importer's base back to `0x290` turns it and two older tests red.

**The rows were drawn from the right field and a loaded game drew none of
them.** `ebf8dd5` draws all four from `c.industry[commodity.index()].next_season`,
and because our `Industry` is a plain struct indexed by commodity, the bad base
could not reach the value — *that* part of the hand-off's worry was unfounded. But
nothing filled the field on load: the importer did not read the word, and the
estimate round that writes it runs at the end of a season. Measured on England
turn one before the fix: fourteen counties, fifty-six forecasts, **all zero**,
which `Ui_DrawDelta` with `mode == 0` draws as nothing. This is C142's defect
exactly — fixed at the producer, never at the load — and it is fixed the same way:
`l2-scenario` now reads `+0x2A8 + c*0x18` into `Industry::next_season`, for C142's
reason that the original restores a memory image and does not recompute on load.
`a_loaded_game_draws_each_industry_rows_own_forecast_on_its_first_frame` asserts
the file's own number in the wood row's own band, and four distinct numbers each in
its own row. No save-format change: `next_season` is already carried.

**Not fixed, and the same shape:** the three farm rows' forecasts —
`herd_change_expected`, `grain_change_expected`, `reclaim_fields_finishing`, county
`+0x258`, `+0x22C` and `+0x20C` — are also zero on England's first frame, measured
in the same probe. Each needs its own invariant against the saves before it is
imported, and none was written here.

**And the Ghidra database still carries the old layout.** `ApplyRecords.java` has
not been re-run, so `tools/oracle/decomp/` will go on printing `industry + 1` for
wood's forecast until somebody does; the decompilation is where this error started
and it is the one place the correction has not reached.

---

**C154 — Building while the game is open: the launcher had
never launched the game, and the free fix protected only builds that follow a commit.**

The player, twice: *"game is closed. You cant rebuild with it open?"* Then the
requirement, once it was put plainly: *"we should be able to build new versions while
I have it open, and that the desktop icon should always open the newest version."* Two
promises, and both of them say *always*.

**What Windows refuses is not the link [V].** rustc links into `deps/` without
complaint. Cargo's last step removes `target\debug\l2-game.exe` to put the new one in
its place, and a running image cannot be removed: *"error: failed to remove file
`…\target\debug\l2-game.exe`"*, then *"Access is denied. (os error 5)"*, exit 101. It **can**
be renamed, and a fresh exe then lands on the original path while the old process runs
on from the renamed image. Every lock below is a hidden `PING.EXE` copied onto the path
under test, which locks the file exactly as a running game does and opens no window.

**Three mechanisms, measured, and they are not substitutes for one another:**

| mechanism | which builds survive a running game | always the newest? | cost |
|---|---|---|---|
| **`tools/run/play.cmd`** — build, then run a *copy* | every one, because nothing it starts ever locks the build output | **yes**: it builds first, and a failed build launches nothing | one 19 MB copy per launch |
| `build.rs` moves a locked exe aside (the default) | only a build that follows a commit, checkout, rebase or `git add` | — | nothing |
| `L2_ALWAYS_UNLOCK=1` — that move before every link | every one, even with the exe run directly | — | **every** build recompiles `l2-game`: 2.6–3.2s against 0.17–0.21s for a no-op |

**The middle row is the trap.** `build.rs` watches only the git directory's `HEAD` and
`index`, and emitting any `rerun-if-changed` replaces cargo's default, so an ordinary
edit relinks the binary without rerunning the script. Measured on a clean fingerprint:
locked exe, one touched source file, plain `cargo build -p l2-game`, **exit 101**. The
same lock with the switch: moved aside, exit 0, and the stand-in still running from the
renamed file. So the default is a safety net rather than an answer, and the switch is
a tax paid by every agent and every `cargo test --workspace` for good, to cover the
minutes somebody has the game open. **The launcher is the primary mechanism** —
`docs/environment.md`, *Building while the game is open*.

**What was wrong in what had been written, every item of it stated confidently:**

1. `build.rs` said *"This is cheap and it is not a rebuild … Measured, not assumed"*,
   six lines below the paragraph that refuted it. It was deleted on `wip/build-unlock`,
   whose commit message records how the first deletion had silently failed.
2. *"5 to 7 seconds against 0.19"*, in two files. No repeated build reached 3.2s.
   **7.03s does reproduce — as the first build after an edit to `build.rs`, which
   recompiles the script too.** One early observation of a different event had been
   quoted as the price of every build.
3. *"every build sweeps every `l2-game.old-*.exe` … collected on the next build after
   the player quits."* A sweep runs when the script runs, which is the middle row of
   the table: not every build.
4. **The launcher could not pass an argument.** `play.ps1` declared `[CmdletBinding()]`
   and ended in `& $copy @args`, and an advanced script has no `$args`. Every argument
   was a binding error before the build even started: *"A positional parameter cannot
   be found that accepts argument 'F:\games\Lords of the Realm II'"*. `l2-game`
   requires a game directory, so **the launcher had never launched the game** — and it
   was one message from being handed to the player as a desktop-shortcut line.
   `docs/plan.md` §3.1 already says it: *a development tool that is quietly broken is
   worse than one that is absent.* This one would have failed at the exact moment it
   was meant to prove itself.
5. And one of this branch's own, caught before it landed: *"the link fails"*, written
   into both files before the error had been read. The error names cargo's remove step.

**A measurement trap worth keeping [V].** A build with `L2_ALWAYS_UNLOCK=1` leaves its
non-existent `rerun-if-changed` path in the unit's stored output, and cargo goes on
rerunning the script until a build *without* the switch re-emits a clean set. The first
"default" measurement on this branch was taken straight after a switched build: it
reran the script, moved the exe aside, succeeded — and was a measurement of the switch.
**The fingerprint is whatever the last run of the script emitted.** Flush it with a
plain build before measuring the default.

**The whole loop, end to end, through `play.cmd` [V].** The game was only ever run
with `--help` (usage, exit 2, no window), pointed at a directory that does not exist
(which `Vfs::push_layer` rejects before any window), or replaced by the hidden stand-in.
Hashes are SHA-256 prefixes, from the code as committed.

| step | result |
|---|---|
| edit, then `play.cmd --help` | exit 2, *"playing l2-game-live.exe"*; the copy is the fresh build (`2DE219…`), not the one before it (`DB8C56…`) |
| the same in a hidden real console, no redirection — how a shortcut runs it | exit 2 |
| a game held open *by the launcher*: `play.cmd -NoBuild -t 127.0.0.1` over the stand-in | `l2-game-live.exe` locked; `target\debug\l2-game.exe` **not** locked |
| while it runs: edit, `cargo build -p l2-game`, `cargo test -p l2-game --test press` | both exit 0, and no `l2-game.old-*` appeared — the rename played no part |
| edit, `play.cmd --help` again, the first game still running | exit 2, *"playing l2-game-live-1.exe"*; that copy is the build the launcher just made (`89C434…`), not the previous output (`509F4B…`) |
| a compile error, then `play.cmd --help` | exit 101, *"build failed - not launching."*; no copy made and every file's hash unchanged |
| stop the first game, then `play.cmd --help` | the launcher exited with its game; the next launch swept both old copies and ran a fresh `l2-game-live.exe`, identical to the build output |
| a `.lnk` holding the shortcut line, run exactly as stored, with a game directory that does not exist and has spaces in it | built, launched the copy, and the game's own error named the whole path intact — run from the repository and from `C:\Windows\Temp` alike, so *Start in* carries nothing |

**Why the stale file is renamed rather than the new one versioned.** Linking
`l2-game-<hash>.exe` would work, and it would break every shortcut already aimed at
`target\debug\l2-game.exe`. Renaming the *stale* copy leaves that path — and every
shortcut on it — exactly as it was.

**No test for either sweep; a shape instead.** Each sweep was two literals that had to
agree: the name `make_room_for_the_link` gives an exe it moves and the prefix its sweep
deletes; the names `play.ps1` copies to and the filter its sweep removes. The way
either goes wrong is those two drifting apart, and **that failure is silent on both
sides** — `build.rs`'s warning counts files it *failed to delete*, and a drifted pattern
fails to *match*, so the count stays at zero while the copies pile up. Each side now
derives both from one constant, so the drift cannot be written (`docs/agents.md`,
*Prefer a shape that cannot be wrong to a check that notices when it is*). The
behaviour that is left was observed rather than tested: in `build.rs` a moved copy was
collected on the first rerun after its process exited; in the launcher a copy still in
use survived its sweep, the next launch took `-1`, and once the game had exited the
launch after that swept both. **[I]:** a test would have had
to fake a Windows image lock inside a build script under cargo, and could have caught
only the drift the constant removes — reasoned, not tried.

---

**C155 — The blank sign column is four pixels wide in `Ui_DrawText` and zero pixels wide
in the measure, and the tree documents the wrong function for both.** **[V]**

`crates/l2-game/src/shell/font.rs`'s `SPACE_ADVANCE` doc says *"`FUN_004014F0`
special-cases `' '` before the table lookup and adds 4; `Glyph_Draw` adds nothing
at all for a zero entry, which is what makes `'@'` an invisible sign column that
still occupies its place in a column of numbers."* The conclusion is right and
the mechanism is not, and the two halves of that sentence contradict each other.

`Ui_DrawText` (`0x00402637`) **never calls `Glyph_Draw`** for a glyph-less
character:

```c
local_c = local_c - 0x20;
if ((&g_glyphWidths)[local_c] == '\0') { local_14 = 4; }   /* not Glyph_Draw */
...
g_drawX = g_drawX + local_14;  g_penAdvance = g_penAdvance + local_14;
```

So **every** zero entry — `' '`, `'@'`, `'` — advances four when drawn.
`Glyph_Draw` (`0x00402A14`) does return 0 for a zero entry; it is simply not on
the path. The **measure**, `FUN_004014F0` (`0x004014F0`), is the one that
distinguishes them: it adds 4 for `0x20` alone and **nothing** for any other zero
entry. A string containing `'@'` therefore draws four pixels wider than it
measures, which matters only where the original centres.

### `Ui_DrawCount` has no lead argument, and `Pen::count` had one

**`Ui_DrawCount` (`0x0041AB67`) hard-codes `'@'` and an empty suffix** —
`Ui_DrawNumber(value, '@', &DAT_004D41F4, x, y, font, colour)`, and `0x004D41F4`
is a NUL read out of the shipped `Lords2.exe` at file offset `0xD23F4` (the run
from `0x004D41F0` is `20 00 00 00 00 00 00 00`, so `Ui_DrawYear` style 3's suffix
is one space and `Ui_DrawCount`'s is empty). **[V]**

`Pen::count` took a `blank_lead: bool` and passed it to `Pen::number`, which maps
`true` to *no lead* and `false` to a space, and hard-codes a trailing `" "` either
way. Both answers were wrong, in opposite halves:

| our flag | sites | what it drew | where it was wrong |
|---|---:|---|---|
| `true` | 11 | `"1000 "` — no lead, invented space | **the digits four pixels left**; the noun right by coincidence |
| `false` | 6 | `" 1000 "` — lead, invented space | the digits right; **the noun four pixels right** |

The eleven include **the campaign menu bar's treasury**, which drew its digits at
`x = 500` where `Screen_DrawMenuBar` (`0x00419C78`) and `Ui_DrawCount` put them at
504 — C127's defect, unfixed at the one number a player looks at every turn. The
lost lead and the invented suffix cancel at the **noun**, which is why nothing
looked wrong.

**Fixed.** `Pen::count` no longer takes a lead: `shell::COUNT_LEAD` and
`shell::COUNT_SUFFIX` carry `Ui_DrawCount`'s own, and `Pen::count_with_noun`
takes a noun already chosen, for callers that keep an English fallback. All 17
`Pen::count` sites, and five more that had built `Ui_DrawCount`'s two draws by
hand, now go through it:

* `army.rs`'s treasury (`0x00417A80` area) and `job.rs`'s worker count — digits
  four left, as `true` was;
* `siege.rs`'s *N Seasons* (`Screen_SiegePrep`, `00420000.c:674`) — noun four
  right, as `false` was. The sentence's tail still lands at `0x50` plus the
  count's width, because the original saves `g_penAdvance = 0x30` across the call;
* `armoury.rs`'s troop count, `&g_fontHeading` — digits **and** noun four left,
  built as `"{held} {noun}"`;
* `battle.rs`'s two totals, `FUN_004224E7`'s `Ui_DrawCount(menTotal, 0x48, …)` —
  **upper-cased** the group 8 noun, so *"Total men"* was a line of `Fntl2_14.pl8`
  blackletter capitals, the illegibility a player reported of the title screen.

And one `Ui_DrawNumber` beside them: `UnitPanel_Draw`'s moves left,
`Ui_DrawNumber(left, '@', &DAT_004D4228, 0xF8, …)` with `DAT_004D4228` a NUL
**[V]**, now `Pen::number_in` with its own lead and suffix.

**The fallback font had to learn `'@'` first.** `crates/l2-view/src/text.rs` drew
a real at-sign for it, so a `Pen` passing the original's lead would have printed
`@1000 Crowns.` on an install with no `Fntl2_*.pl8`. It is now `BLANK` and still
advances its cell.

### The court was drawn in the wrong face

`Court_Draw` (`0x00416925`) passes **`&g_fontHeading`** to all four store values —
`Ui_DrawCount(gold, 0, 0xE0, 0x72, …)` and `Ui_DrawNumber(iron|stone|wood, ' ',
" ", 0xE0, y, …)` — beside labels that are also heading. We drew the values in
body, and the three materials with no lead. The suffixes are `&DAT_004D3F8C`,
`…90`, `…94`, each one space **[V]**. `shell::Face` exists so that a count can say
which face its call site names; `Pen::count_in(Face::Heading, …)` and
`Pen::number_in` carry it.

### Tests, each ablated

* `chrome_text::the_treasury_s_digits_start_one_sign_column_right_of_ui_drawcount_s_x`
  — the digits found **on row 6** in `Fntl2_14.pl8` at **504**, and *"Crowns."* at
  `504 + width + 4` = **547**. Every expected number is a literal from the three
  functions, not a constant of ours. Ablated: the old `"{value} "` string → digits
  found at **500**; `COUNT_SUFFIX` → `" "` → noun found at **551**.
* `chrome_text::the_court_s_stores_are_drawn_in_the_heading_face_at_court_draw_s_x`
  — gold and iron found in `Fntl2_22.pl8` on rows `0x72` and `0x90` at `0xE4`.
  Ablated: `Face::Body` → **not found**.
* `l2_view::text::tests::the_at_sign_holds_a_column_and_paints_nothing` — `"@1"`
  equals `"1"` drawn one cell right. Ablated: the bitmap restored → the first
  cell paints.

The row-pinned search is deliberate: a whole-canvas search returns the first
match in raster order, and a short number is usually a digit inside some other
number higher up.

### Measured and not fixed

**`Ui_DrawCount`: 80 call sites, not 81.** 80 `CALL 0x0041AB67` in the shipped exe
and 80 call lines in the decompilation. `docs/draws-map.md` §5.12 said 81 and a
brief quoted it. **Faces: 78 body, 2 heading** — 70 name `&g_fontBody` at the
call, 8 are inside `FUN_004224E7` whose three callers all pass `&g_fontBody`, and
the two heading ones are the court and the armoury above. Our 22 reproductions
account for about 26 of the 80 original calls; the other ~54 — the county job
and farm panels (`00410000.c:1092–1420`), the tile panel's rows (`4649–4982`), the
message scrolls (`00470000.c:2087–2300`) — were **not traced** to a draw of ours
in this pass. **[I]** that they share the defect wherever we draw them by hand.

**`Ui_DrawNumber`'s suffixes, which C127 swept the leads of and not these.**
Every suffix pointer resolved to its bytes in the exe:

| lead | suffix | calls |
|---|---|---:|
| `' '` | `" "` | 76 |
| `'@'` | `""` | **37** |
| `'@'` | `" "` | 20 |
| `' '` | `" %"` / `"%"` | 6 / 5 |
| `'@'` | `"%"` | 5 |
| `' '` | `"- "` | 5 |
| `'('` | `")"` | 3 |
| computed | computed | 4 |
| `' '` | debug captions — `" h1"`, `" Dchk"`, `" divergances"`, … | 29 |

190 parsed, against **211** `CALL 0x00402F64` in the exe — 21 the corpus parse did
not reach, unexplained. Faces: 117 `&g_fontBody`, **44 `&g_font8`**, 13
`&g_fontHeading`, 3 `&g_fontSmall`, 2 `&g_font10`, 11 passed through. **`g_font8`
is a fourth face at 44 sites and this workspace loads three**; which file it is,
and which screens, is not established.

**Our `Pen::number` is unchanged: 25 sites, 19 `true` and 6 `false`,** still
dropping `'@'` and still inventing `" "`. By the census, `'@'` is the lead at 62 of
190 and the empty string is the suffix at 38. Each needs its call site read,
because a caller that chains a sentence off the return moves when either changes.
`Pen::number_in` is the shape.

* **`Font::width` charges 4 for `'@'` where `FUN_004014F0` charges 0.** Only
  visible under centring; no centred draw of ours passes `'@'` — by search, not by
  test.
* **`UnitPanel_Draw`'s year formed is `Ui_DrawYear(…, style 0)`**, which appends
  `L2.eng` group 26's *"AD"*; `info.rs` draws the bare number. Rule 6.
* **`battle.rs`'s roster rows still upper-case their troop nouns** — the same
  blackletter capitals the totals had.

**The reports that started this were stale.** *"Still placeholder font in the top
right for gold and summer"*, the illegible build stamp and the illegible title
were all fixed by `c06b13b`; all four render correctly at `5338fe7` and were
looked at. `build_id.rs` exists to make that checkable from a screenshot — read
the stamp off the report before writing the brief.

**Superseded by C157, below:** *"Our `Pen::number` is unchanged"* was true when written.
The method is now deleted, and each of its 25 sites has been read against its original.

---

**C156 — The click is two sites, not four.**

**Written in two sittings with a restart between**, and the second sitting
compiled, wired and tested what the first could only record. The evidence below
was `[V]` from the start; the click it was written for now sounds, and
`tests/click.rs` asserts when it does not.

`docs/audio-triggers.md` closes with a correction it is proud of:

> *"The pointer click is not one call site behind one hit-tester. It is four,
> behind three … whoever builds the shared widget layer has **three** functions
> to reconcile, not one."*

**It is two, behind one.** `FUN_0040D6AD` and `FUN_0040D7B8` — the other two
sites — are the decrement and increment arrows of a **slider widget that the
shipped game never instantiates**. Their only caller is `FUN_0040D3F5`, the
slider's hit-tester, whose sibling `FUN_0040D271` is its painter
(`g_systemSheet` frames `0x4A`, `0x4B`, `0x4C` — two arrows and a thumb).
**Nothing in `Lords2.exe` calls either.** `[V]`, from the player's own
executable:

| scan over the image | `FUN_0040D3F5` | `FUN_0040D271` | control: `Widget_Test` |
|---|---:|---:|---:|
| `E8` rel32 CALL, all of `.text` | 0 | 0 | **36** |
| `E9`/`EB`/`0F 8x` jumps, all of `.text` | 0 | 0 | — |
| absolute dword, whole image | 0 | 0 | 0 |

**The control is the point.** `Widget_Test` has 36 callers and scores zero on the
absolute scan, because this binary calls with `E8` rel32 — so a bare
absolute-address scan finding nothing is not evidence of anything. Run the
control, every time.

Two lessons, and the second is the one worth carrying:

* **A correction is a claim and inherits every obligation a claim has.** This one
  replaced a true sentence — *"one call site, because the whole game shares one
  hit-tester"* — with a false one, and it was more confident than what it
  replaced. `CLAUDE.md` rule 4 does not exempt the correction log.
* **Counting call sites is not counting reachable call sites**, and
  `docs/audio.json` had no way to say so, because its `dead` column was asserted
  empty. That assertion is right to exist — a bucket that fills quietly stops
  being a finding — and these are the first two entries it should cost a decision
  to add.

A detail worth keeping even though the widget is dead: inside `FUN_0040D3F5` the
**two arrows click and the thumb drag does not** (`FUN_0040D5A0` is gated on
`g_mouseLeftDown` and plays nothing), which is the same rule `docs/input.md` §5
already records for drags.

**And what the click actually is**, which `docs/input.md` §5 had right all along
and `docs/audio-triggers.md` did not: `Sound_RestartSlot(1)` — `click3.wav`, slot
1 of *both* banks — from `Widget_Test`'s **kind-4** and **kind-5** arms, both
guarded by `g_mouseLeftPressed || g_mouseLeftDoubleClick`, **on the initial press
only**. The auto-repeat's later pulses are silent, kind 2's toggle is silent, and
`Hotspot_Test` and `Ui_OkButtonClicked` — the majority of the interface — are
silent. `[V]`, `0x0040DA1E`. Getting that wrong is audible in a way the trigger
count cannot see: a spinner that clicks thirty-three times a second is the same
"one reproduced trigger" as one that clicks once.

**The blocked 55, with the second column**, because it is the part that dies
otherwise. Weights are distinct install `.wav` files each group can reach:
battlefield 25 sites / ≤17 files (the 66 troop-cry files are filed `missing`, not
blocked); Smacker 8 / ≈0 new; sibling voice tables 6 / ≈32; the field brush 5 / 3;
**the hit-tester 4 / 1, of which 2 are dead**; the battle verdict 2 / 1; the two
delegated painters 2 / 0 new; **the chained takes 1 / 27**; the mercenary offer
1 / 12; the lord sting 1 / 4. The group to take next is the **chained takes** —
one site, 27 files, the largest files-per-site ratio in the inventory, and both
halves of what it needs already exist in shape (`Mixer::is_playing`, and a
`Director` that already keeps per-tick memory).

**That proposal was wrong about its blocker, and reading the call site showed
why.** `FUN_004B3ACD` has one caller — `Msg_DrawWindow`'s categories
`0x05`…`0x09` branch — and its cursor is reset in one place, `Tip_Show`. Those
categories are the **tip screens**, and `Tip_Show` is the only function in the
original that posts one. Our engine posts none. So the chain is not blocked on a
cursor; it is blocked on a screen, and **it took a reproduced claim down with it**:
`Msg_DrawWindow#24`, the same branch's first line, had been `reproduced` since the
narrator landed, and thirteen tip clips were in the *"543 reachable"* because a
name-driven loop resolved their names. Both are corrected — 51 of 143, 560 of 771.

The lesson is the one `docs/audio-triggers.md` already carried about the ninth
primitive, pointed the other way: **a count measured by resolving names measures
the names.** The test that produced 543 drove `message_voice` over every group
in the bands, which proves a file exists for each and says nothing about whether
the game can post the message. Only the tip band has been checked; the rest of
the 530 has not been re-audited against what actually posts.

Two smaller corrections from the same reading: the chain was said to be *"why
`S010_13.wav` exists"*, and that clip is `g_msgVoiceS010`'s; and the lord sting's
table at `0x004E2470` holds four clips and a sentinel, not five lords.

---

**C157 — The original preloads five faces, and the fourth draws nothing a player sees.**

**`g_font8` is `Fnt_8.pl8`, and every use of it in the binary is a developer
read-out.** So no draw of ours was switched to it. **[V]**, from the bytes.

* **The loader.** `Res_LoadStatic` (`0x00499859`) hands record `n` of
  `g_preloadTable` (`0x004D9F48`, twenty-byte `{name[16]; size}`) to
  `File_ReadChunk`. Record 3, at `0x004D9F84`, is `"fnt_8.pl8"` with a size of 5,200.
  The arm that selects `&g_font8` for `n == 3` is `C7 45 FC B0 BF 5C 00`,
  `mov [ebp-4], 0x005CBFB0`, at `0x004998ED`.
  `tests/shell.rs::the_preload_table_names_every_face_and_record_3_is_g_font8`
  asserts both.
* **The uses.** `.text` holds 86 four-byte references to `0x005CBFB0`. One is
  that loader line. The other 85 are all `push` operands, inside six functions:
  `Net_DrawDebugOverlay` (`0x00423BA4`), `BattleDebug_Panel` (`0x00424992`),
  `FUN_00425314`, `FUN_00425487`, `FUN_0042563C` and `FUN_00425799`. Their
  captions are `" divergances"`, `" Dchk"`, `"FIGURE"`, `"GROUP"` and
  `" p1 rank"`. The decompilation agrees: 85 lines in `00420000.c`, one in
  `00490000.c`. `symbols.json`'s *"the battle overlay"* is half of it; the
  network overlay is the other half.
* **Loaded anyway.** `ShellAssets::eight` and `shell::Face::Eight` exist, and
  `missing_fonts` names `Fnt_8.pl8`. The one-baseline test runs over it, and it
  holds (all 150 frames have `0x0D == 0`, so the test's overhang-split half does
  not apply).

### The brief said four faces. The preload table has five.

Records 3…7 are `fnt_8`, `fntl2_9`, `font_10`, `fntl2_14`, `fntl2_22`.
**`Font_10.pl8` (`g_font10`, `0x005AEBA0`) is still not loaded**, and unlike
`g_font8` a player sees it. Its nine `.text` references are
`FUN_004100AF`, `FUN_0041023A`, `FUN_004103C5` (two), `FUN_00410502`,
`FUN_00410598`, `FUN_0041062E`, `FUN_004106C4` and
`CountyStrip_DrawCastleIcon`: the county strip's produce rows and castle cell.
`screens::county::strip_delta` draws them in `Fntl2_9.pl8` and says so.

It was not loaded here for a measured reason. Pointing `font::EIGHT` at
`Font_10.pl8` made the baseline test panic: a lowercase letter drew nothing
through `GLYPH_MAP`. The file is 3,342 bytes, and `pl8.md` counts 7 of its
frames as declaring rows. **[I]** It is not an alphabet under the shared table,
so loading it starts with reading `Glyph_Draw` against that file.

`g_fontSmall`'s nine references include one outside the strip:
`Screen_DrawEndTurn`'s `Ui_DrawCentred(4, 0, 0x1DE, 0x1CE, 0xA2, &g_fontSmall,
0x16)`. So `symbols.json`'s *"the county strip, and nothing else"* is wrong by
one. It is not edited here, and `docs/screens-county.md` §4 now carries all three
rows with their reference counts.

### `Ui_DrawNumber`'s 21 unexplained call sites are unreachable code **[V]**

There are 211 `CALL 0x00402F64` in the shipped exe and 190 parsed calls in the
corpus. The 21 missing calls come in two groups.

* **10 are in `Diplo_DrawLordCard`, past where Ghidra ends it.** Ghidra's body
  stops at `0x004175F2`. The code at `0x004175ED…0x00417896` follows an
  unconditional `jmp 0x00417896` at `0x004175E8`, which is the function's own
  epilogue.
  * Every call has lead `' '`, `&g_fontBody` and colour `0x20`.
  * Suffixes: `"t"`, `"p"`, `"m"`, `"r"`, `"s"`, `"a"`, `"o"`, `" ranked"`,
    `" strength"`, `" ally"`.
* **11 are in `Screen_DrawMenuBar` (`0x00419C78`), in three blocks.** The
  entry path skips all three: `jmp 0x00419D88` → `jmp 0x00419E22` →
  `jmp 0x00419ED4`.
  * Every call has lead `' '`, `&g_fontBody` and colour `0x3F`, at y 1 to `0x22`.
  * Suffixes: `" b phys"`, `" free"`, `" b page"`, `" free"`, `" mb virt"`,
    `" mb free"`, `" time"`, `" crc"`, `"end"`, `"test"`, `"max players"`.

No `rel32`, `jcc32` or `rel8` branch anywhere in `.text` targets `0x004175ED`,
`0x00419C86`, `0x00419D8D` or `0x00419E27`. No absolute copy of any of those
four addresses exists anywhere in the file. So the corpus's 190 is the whole
live population, and the decompiler was right to leave the 21 out.

### `Pen::number`'s 25 sites, each read — and the method deleted **[V]**

Every suffix pointer was resolved to its bytes in the image.

| site | original | lead, suffix | what was wrong → what moved |
|---|---|---|---|
| `armoury.rs` rack count | `Armoury_DrawRacks` | `'@'`, `""` | no lead: digits +4 |
| `army.rs` population, levy | `Screen_RaiseArmy` | `'@'`, `""` ×2 | digits +4 |
| `army.rs` happiness | 〃 | `'@'`, `""` | digits +4; **face icon unchanged** |
| `army.rs` price, wages | 〃 | `'@'`, `""` ×2 | digits +4; **both nouns unchanged** |
| `army.rs` weapon total | 〃 | `'@'`, `""` | digits +4; **noun unchanged** |
| `castle.rs` stone, wood | `Screen_CastleBuildPanel` | `'@'`, `""` ×2 | digits +4 |
| `castle.rs` garrison | 〃 | `'@'`, `" "` | digits +4 **and** *"troops."* +4 |
| `divide.rs` rows, band, parent total | `Screen_SplitArmyRows` | `'@'`, `""` ×5 | digits +4 |
| `message.rs` room | `Msg_DrawWindow` | `'@'`, `" "` | digits +4 **and** noun +4 |
| `message.rs` men | 〃 | `'@'`, `" "` | digits +4 |
| `merchant.rs` stock | `Trade_DrawPanel` | `'@'`, `""` | digits +4; **icon unchanged** |
| `merchant.rs` quantity | 〃 | `'@'`, `""` | digits +4 |
| `army.rs` rack stock | `Screen_RaiseArmy` | `' '`, `" "` | already right |
| `map.rs` year | `Ui_DrawYear` style 3 | `' '`, `" "` | already right |
| `supplies.rs` ×2 | `FUN_0041AEA2` | `' '`, `" "` | already right |
| `ratings.rs` score | `Screen_BattleMasterRatings` | `' '`, `" "` | **face**: heading, not body |
| `info.rs` formed | `Ui_DrawYear` style **0** | `' '`, `" "` | **missing 26/1 *"AD"*** (rule 6) |

**Counts: 21 fixed, 4 already right, and none of the 25 unexplained.** The
fixes are 19 leads, 1 face and 1 era.

**The pattern that hid it:** wherever the suffix is empty and text follows, the
lost `'@'` and the invented `" "` cancel at the text. So the words sat right and
only the digits were four pixels off. The two sites whose suffix really is
`" "` moved their words too.

`Pen::year` reproduces `Ui_DrawYear`'s four styles; all eight of its suffixes are
one space. `Pen::number` is gone. A choice the original does not offer cannot be
made correctly.

**Found on the same screens while reading them, and fixed**, because they are
the same defect built by hand:

* `army.rs`'s band count was `"{men} "` in the heading face. It is `'@'`, `""`.
* `siege.rs`'s engine percentage was `"{p}%"`, and its comment said no `Pen`
  could carry the lead. It is `'@'`, `"%"`.
* `divide.rs`'s daughter total was **two identical `text::draw_right` calls in
  our 5 × 7 font**, in highlight ink. It is `'@'`, `""` in body.
* `battle.rs`'s roster:
  * The rows were `n.to_string()` with no lead. `FUN_004224E7` passes `' '`,
    `" "`, so both columns move +4.
  * The `"(was)"` column was drawn in `font::DISABLED`, where the original passes
    `0x3F`.
  * The nouns are no longer upper-cased.

**`Font::width` charges `'@'` nothing** now, as `FUN_004014F0` does.

The draw audit's `OURS` map loses `number` and gains `number_in`, `count_in`,
`count_with_noun`, `text_in` and `year`. The old `number` key had also been
counting `page.number()` in `setup.rs` and `index.rs`, which draw nothing.
`draws-ours` goes 418 → 411.

### Tests, each ablated

| test | ablation | red with |
|---|---|---|
| `chrome_text::the_castle_s_garrison_and_its_noun_both_start_one_sign_column_right` | old `"{cap} "` | digits at 12, expected 16 |
| 〃 | suffix `" "` → `""` | noun at 51, expected 55 |
| `chrome_text::the_mercenary_price_line_moves_its_numbers_and_not_its_nouns` | old `"{price} "` | price at 112, expected 116 |
| 〃 | suffix `""` → `" "` | *"crowns to hire."* at 164, expected 160 |
| `chrome_text::the_unit_panel_says_the_year_an_army_was_formed_in_ad` | style 0 → 3 | *"AD"* not found |
| `chrome_text::the_battle_master_score_is_in_the_heading_face` | `Face::Body` | not found in heading |
| `shell::the_measure_charges_the_blank_sign_column_nothing_and_the_draw_charges_four` | `None => SPACE_ADVANCE` | 43, expected 39 |
| `shell::the_preload_table_names_every_face_and_record_3_is_g_font8` | `EIGHT = "Font_10.pl8"` | record 3 is `fnt_8.pl8` |

**Not tested on a canvas, and changed by reading alone:** `message.rs`,
`divide.rs`, `merchant.rs`, `siege.rs`, `supplies.rs`, `armoury.rs` and
`battle.rs`'s roster. The roster needs a pending battle report, and none of the
fixture-gated suites builds one with fonts loaded.

**Stale prose deliberately not edited:**

* `docs/plan.md:135` still describes `Pen::number`.
* `symbols.json`'s `g_font8` and `g_fontSmall` comments are incomplete, as above.

---

**C158 — The turn timer is a limit on the person's own turn, in single
player too, and the guard two documents gave it was half of an `||`.**

Found beside the shield row, which reads the same flag, and listed in C140 as one of
`Ui_DrawNumberRight`'s undrawn sites. Nobody had reported it and nobody had asked what
it was for.

**What it is.** **[D]** unless marked. A time limit on the person's own turn. The
setting is the custom game's *Time limit* drop-down, and the **single-player** page
(`g_setupPage` 7, `FUN_0041F86D(999)`) draws it with the other eleven;
`Setup_CommitOptions` turns the index into seconds through `g_timeLimitSeconds`;
`Setup_DefaultOptions` picks *no limit* for one player and *4 mins* for a network game;
`Campaign_LoadEntry` forces 0. So it is **not multiplayer-only and not a display** — a
single-player custom game with a limit chosen has a countdown that ends turns — which is
why this was built rather than routed to `docs/netcode.md`. Its network half (a
2.5-second restart delay, a ten-second `Turn_End` resend) is not ported, and would be
that document's to design.

**What happens at zero.** `Turn_Tick`'s phase-4 arm (`0x0049A010`) recomputes
`DAT_005440C8 = g_optTimeLimit − (timeGetTime() − _DAT_00568D9C) / 1000` and below zero
writes −1 and calls `Turn_End` — the End Turn button's handler, which ends **the local
player's** turn and nobody else's. A 30-second limit therefore ends the turn when **31**
seconds have passed, and `0` is never drawn. `Turn_End` dismisses an open message; from
the next frame `Screen_FrameInput` closes every screen whose arm carries the turn-ended
guard. The count restarts on the first frame of the person's next turn, in the same arm.

**Where it is drawn.** Not by a painter: `Battle_Frame` calls `FUN_0041A639` near the
end of its tail. `Misc_cty` frame `0x60` at (404, 430), and
`Ui_DrawNumberRight(DAT_005440C8, ' ', &DAT_004D41D0, 0x1A8, 0x1BA, 0x32, &g_fontBody,
0x3F)`. `&DAT_004D41D0` is `" "` **[V]**, read from `.data`; the function centres
(C119) and measures the whole buffer (C140), so it is `" 30 "` centred in fifty pixels.
Which screens it is drawn over is `DAT_004D2E80[g_screenId] == 0`, seventy bytes read
out of `.data` **[V]** with **one reader in the whole binary** — the timer's own
vocabulary: the map and its insets yes, the pages no.

**The guard was written down backwards.** C140's table row and `docs/draws-map.md` §5.11
both gave it as `g_optTimeLimit > 0 && aiStep == 999`. The clause is
`(DAT_0055403C < 1 || g_realms[g_localPlayer].aiStep == 999)`. `DAT_0055403C` is what
`Turn_End` writes and the restart clears, so it is 0 through the person's own turn and
the first half is what puts the timer up while he plays; the second half keeps the
frozen number up after he ends it. A port of the documented guard draws a countdown only
once there is nothing left to count. It is `docs/agents.md`'s *a true statement about one
branch, promoted* — the branch this time being one operand of an `||`. §5.11 is
corrected; C140's row is left for the integrator.

**The trap is the one C152 found, and so is the mapping.** Our
turn is the original's rotated, so nothing here reads `ai_step`. The person's
`aiStep == 999` is `turn::players_turn_ended` — new, because `turn_in_flight` is also true
for a battle raised on an ordinary frame, and `Turn_Tick` counts on under that prompt.

**What was built.** `crate::turn_clock`: the countdown as a state machine over a `Frame`
that reads nothing else, the screen table, and the two draws. `Machine::run_turn_clock`
and `Machine::draw_turn_timer`, because the original's call sites are the frame loop;
`MapScreen::update` carries the `Turn_End` out through the button's own `end_turn`,
because only the map starts a turn here; the phase-2 pump clears the restart flag as
`Turn_Tick` does before every assault. Session state on `Game` — not in the save, which
is `Setup_StartGame`'s own behaviour on a load, and not in the digest. `docs/arms.json`
`0x0049A010/turn-time-limit`; `docs/draws-map.md` 61 of 121.

**A defect came with it, and it is reproduced.** The count runs *before* the restart in
the same frame and the start is not moved while a turn runs, so ending a turn early and
waiting on a slow one can end the next turn on its first frame. `docs/bugs.md`
B99, **[I]** on reachability, and `docs/oracle-requests.md` §12
is the thirty-second observation that settles it.

**And a count in the inventory was high.** `docs/arms.json`'s
`0x0042FF10/force-close-on-turn-end` said twenty-nine arms close; there are twenty-seven
sites of the guard, twenty-six of which close. The timer makes that arm reachable in
single player for the first time; the pop loop reproduces it for the interval between the
clock's `Turn_End` and the map starting the turn, and the record stays `missing` for the
turn itself.

**Not reproduced**, each said in the module header: the count running on through the rest
of phase 4 after the click, and the timer hiding during phases 1 … 3 — both artefacts of
the rotation; the turn running behind a screen the guard does not close (ours waits for
the map); `0x13` closed by the guard (ours would push it straight back);
`FUN_004976A1`'s `+10` on an in-game load.

**Tests.** Eight in the module, driven by `Frame` values alone; two through a whole
`Machine` on the England fixture; one reading the table and the suffix out of
`Lords2.exe`. Every expected literal is the decompilation's, not the module's. Seven
ablations, each observed red: the guard as documented; the suffix emptied; the request
deleted; the person's turn read off his `ai_step`; the pop loop deleted; `0x0B`'s byte
zeroed; the restart moved above the count — which turns only the defect's own test red.

---

**C159 — the numbering tool skipped what it could not read, and its
first run over `docs/bugs.md` found a duplicate on `main` that nothing had.**

Merging six branches, the integrator closed three gaps in
`tools/decisions/corrections.js` by judgement. Two branches had written their
correction as a bare Markdown heading over its placeholder, which the tool did not
recognise as an entry, and both were rewritten by hand. A branch had written a
dead-code entry in `docs/bugs.md` with a `D` placeholder; the tool scanned for `C`
and `B` placeholders only, so nothing reported it, and it became D39 by hand. And a
branch's bug row, written as a placeholder, was invisible to `quirks_catalogue.rs`,
whose parser accepted digits only: the branch was green, and two tests went red at
merge the moment the row had a number. Assignment itself was `git grep -l | perl
-pi`, while the tool's own failure message said it was *"one command per tag"*.
**[V]**, all four, by reading the tool and the catalogue as they stood at `9cbd3af`.

**The three are one mistake.** Each check matched the good shape and passed over
everything else in silence, so a wrong shape was not a failure — it was an absence,
and an absence passes. C146 vanished the same way (C147). And for a numbered log an
invisible entry is worse than a missing one: the next free number is computed
without it, so the next assignment collides with it.

**What changed.** `docs/agents.md`, *The tool as it is*, describes the result; in
short:

* **Six series, each a *(log, letter)* pair** — D and C in `decisions.md`; B, N, S
  and D in `bugs.md`. The D-series exists twice and the two are unrelated, so a `D`
  placeholder takes its number from the log its entry is in. Every series is
  duplicate-checked, and a placeholder of a letter neither log numbers is reported.
* **A line shaped like an entry and not in its log's form is an error naming the
  line**: a Markdown heading in `decisions.md`, the wrong heading level, a hyphen,
  en-dash or colon for the em-dash, the em-dash double-encoded, a cell closed tight.
  Prose that opens with a bold id — both logs have plenty — stays quiet.
* **`--assign <TAG>`** takes the next free number in the tag's own series, replaces
  it in the tree's bytes rather than its decoded text, and relocks. It refuses and
  writes nothing when the tag is absent or undefined, when the tree holds a drag the
  relock would accept, or when either log has a malformed entry line — so it never
  numbers from a log it cannot fully read.
* **`--check` reports placeholders last**, each with its defining line, its command
  and the number it would take. The old order exited on a placeholder before the
  lockfile rules ran, and before `--relock` could run at all, so a branch carrying
  its own placeholder could neither verify nor regenerate its lock. **[V]**, from
  the order of the old source.
* **`quirks_catalogue.rs` accepts a placeholder as an entry**, so an unwired row
  fails on its own branch, and `--assign` renames the `DISPOSITIONS` line in the
  same pass as the document.

**The duplicate.** Generalising rule 1 to every series failed on its first run:
`docs/bugs.md` had two `B69`s — a §2.5 table row, *"whether you can order an attack
depends on a figure index left over from another sweep"*, and the §2.10 heading on
the siege repair bill. The catalogue read both and held one disposition for the
pair, so it passed. **[V]** by `git log -S`: the heading arrived in `66d88a5`, the
row about an hour later in `dd1918c`. The later arrival is now **B100** (B99 was taken by the turn timer's row by the time this merged), with its two
citations, `battlefield.rs::update_hover` and the `Battle_UpdateHover` arm's note in
`docs/arms.json`, and its own `DISPOSITIONS` row. The other nine `B69` citations
name the repair bill and stay — eight by what they say, and `runner.rs`'s
wall-collapse comment **[I]** by its subject, the wall-damage count that entry bills.
`docs/agents.md` records the B-numbers as caught by the quirks catalogue; this one
was not, because a join on ids cannot see two entries sharing one.

**Ablated, every test**, by editing the tool or parser and running the file, and
every ablation went red where its test says it will. Two results were not what the
first draft of the tests' comments claimed, and the comments now say what happened:
keying duplicates by id without the log fails all eleven tool tests rather than one,
because every test tree carries D1 in both logs; and narrowing the placeholder scan
to `C` and `B` still lists defined `D`, `N` and `S` entries, which come from the log
parse, so only the stray-letter assertion and the dead-code assignment carry it. The
one that matters most for the catalogue: with a placeholder row added to the real
`docs/bugs.md` §2, the current parser fails, naming the probe's tag as a row
*"DISPOSITIONS does not"* have, and the old digits-only parser passes.

**What this does not do.** Citations are checked for C-numbers only; a `B69` that
points at the wrong bug is still found by reading. The entry-shape check covers the
two logs, so an entry written into some other document is caught only if it carries
a placeholder. `--assign` numbers one tag per call. And the catalogue reads §2
only, so a placeholder row in §3–§5 has no test to fail — there is no switch list
for those sections to join.

---

**C160 — The tip screens are built — `crates/l2-game/src/tip.rs` — and four things on file
about them were wrong.** Each was a sentence a careful person would have built from.

**One: the twenty frames are not "after a screen is first opened".** `docs/symbols.md`
(`Tip_Update`) and `docs/formats/eng.md` §5.3 both said so. `DAT_004F0358` has exactly
two writers, `FUN_00476A5D` and `FUN_00476E21`, so the delay is a **re-arm** — after
start-up, after the toggle, and after every dismissal while `g_screenId` is `0x27` —
and a screen opened with the counter at zero gets its tip on that frame. `[V]`
`tests/tips.rs` counts the 21 ticks, typed.

**Two: "once per game" is once per run.** `FUN_00476A5D`'s only callers are
`App_WinMain` and `Opt_ToggleTipScreens`; `Game_NewGame` does not clear `g_tipShown`
and neither does a load. So `Game::tips` is carried across the two places this engine
replaces a whole `Game`. `[V]`

**Three: a tip is not a window, it is a screen.** `Tip_Show` writes `g_screenId =
0x27` and posts; the window follows only because `Msg_Pump` runs on `0x27`, and the
screen underneath stops answering because `Screen_FrameInput` dispatches on the byte.
`ScreenId::Tip` is that screen. The decompilation compares `g_screenId` with `0x27`
only in `Msg_Pump`, `Tip_Show` and `FUN_00476E21` `[V]`, so it has no arm and no
painter a comparison shows `[I]`.

**Four: the OK button has no widget record.** The brief for this work said its
gesture would come from one. `node tools/oracle/kinds.js` lists no record for
`Msg_HandleInput` or `Ui_OkButton`, because the message scroll's corner is
`Rect_Contains` round the last drawn `Ui_OkButton` on `g_mouseLeftPressed` — a
**left press**, already `0x0047685D/message-ok-dismiss`. What was missing was the
frame: `message::frame_of` has no row for categories `0x05`…`0x09`, so before this a
tip window could not have been closed with the left button at all. Its corner is
computed from the wrapped text — `message::paragraph_layout`, with
`FUN_0040328E`'s own line breaking, `message::break_lines`.

**The two arms the ladder could not see, resolved rather than approximated.**
*Army movement* is `g_screenId 0x10`, written only by `Map_BeginMoveSelection`; ours
is `MapScreen::move_order`, now askable as `Screen::mode_screen_id`. *Invasions* is
`DAT_00553210`, set by `Unit_EnterCounty` — whose only caller is `Army_Tick` — when
the county's owner is not the army's and the army's owner is `g_localPlayer`. The
world half is reported by `l2-kingdom` as `units_tick::Incursion` at the crossing;
the local-player half and the flag are `l2-game`'s. Nothing new is saved.

**Three of fourteen tips cannot be posted, in the original either `[I]`.** 212, 214
and 215 need `g_screenId == 0 && g_battlePhase == 2`; every `g_battlePhase = 2` write
found sets `0x29` beside it, and `Msg_Pump` dismisses any message in phase 2. Built as
written, and unreachable here too. Five shipped tip clips therefore stay silent.

**And one defect of the game's**, reproduced: the invasion arm clears its flag before
asking whether the tip was shown, so a crossing noticed on screen `0x27` loses the tip.
`docs/bugs.md` B101.

**Found and not this branch's to fix.** Every `Opt_Toggle*` row is a `Widget_Test`
**kind 5** record in `kinds.js` (`0x004DDC10`…`0x004DDD18`, left-press-delayed), our
options screen acts on the click, and none of the rows is in `docs/arms.json` — so
the gesture is wrong and nothing counts it. And `0x00472E46/battle-phase-swallows-messages`
is still `missing`.

**What it cost the suite**, which is the finding about the suite rather than the
tips: sixteen existing tests went red, in six files, because a new game now opens
with three tips that hold the campaign map's input — faithfully — and every one of
those tests ended a turn or ticked the map past frame 21. Each fixture now sets
*Tip screens: No* explicitly and says why, rather than the phase mapping being bent
to hide tips from a machine rooted on the campaign.

Counts: arms, two frame arms `reproduced` (`0x00476AA7/tip-screen-ladder`,
`0x00476E21/tip-restores-its-screen`); sound triggers **51 → 53 of 143**; files
**560 → 595 of 771** — forty by name, thirty-five that can sound.

---

**C161 — "a stored field our importer drops" is now a red test,
and the inventory that makes it one found 153 of 241 undecided.**

C142 and C153 were one defect found twice by a player, with the
farm-row forecasts measured as the third. Each field was stored by `Lords2.exe`,
modelled by `l2_kingdom`, carried by our own save format, drawn by our painter —
and never read out of a `.sav`. C142 named why the existing defence could not see
them: **the exhaustive destructure guards `CountyState`, and a field never added
to `CountyState` is not in the list it enumerates.** So the fix is not a fourth
field. It is a denominator that belongs to the original.

**The inventory.** `docs/stored-fields.json`, one row per field of the county and
realm records: `imported`, `derived`, or `excluded` with a one-line `why`, and no
fourth status. Its rows come from three places, and each is checked:

* **The original's side** — `node tools/oracle/fields.js`, new, reads every
  `g_counties[i].x`, `.field_0xNNN`, `i * 0x300 + 0x53fXXX` and absolute `DAT_`
  access in the decompilation, and the same four shapes for `g_realms`, and lists
  each offset with the functions that write and read it. **318 accessed offsets,
  221 county and 97 realm**, plus three member names typed over the wrong array
  that it reports and does not count. `--check` fails on an offset no row covers
  and on a row no instruction touches.
* **The layout** — every field `docs/records.json` names must have a row of that
  name, width and array shape (146 fields once the nested arrays expand).
* **The saves** — `crates/l2-scenario/tests/stored_fields.rs` holds every
  `imported` and `derived` row to the file's own value after `Scenario::kingdom`,
  in every county and realm of every save on the machine: **30,600 values over
  18 saves, and all agree.** That is the check that cannot be typed into
  agreement, and it is the one that would have caught all three instances.

**Before and after, in rows.** At `d2344f3`: **76 imported, 10 derived, 2 excluded
with a reason** (C142's county `+0x16` and realm `+0x28`, left open in prose), and
**153 decided by nothing**. After: **167 imported, 10 derived, 64 excluded, 0
undecided.** Two of the 153 were worse than dropped — the importer set
`happinessAvg` and `happinessSum` to this season's happiness, which is right on turn
one and wrong on every later turn (`siege-aftersie.sav` county 2 draws 95 where the
original draws 54).

**What a player could see, now carried.** The three farm rows' forecasts (county
`+0x258`/`+0x268`/`+0x26C`, `+0x22C`/`+0x230`/`+0x2FC`, `+0x20C`/`+0x214`); the
happiness panel's average, army, ale and other-counties terms; the population
panel's *Army* line and emigrant destination; the ration panel's troop counts
(`+0x198`, `+0x19C` — 178 and 360 on the battle saves, 0 on load); the weapon the
blacksmith row draws (`+0x290`); the industry ramp (`+0x294` — **80 in every owned
county of every save, and a loaded game ran its mines at `Industry::new()`'s 20 and
15**); the levy surcharge; the castle build record and siege scars; the standing
crop; and on the realm, the ally byte the diplomacy screen draws, the score
screen's totals and the AI's standing orders.

**Self-verifying invariants, all `[V]` across every save** and each asserted in
`crates/l2-scenario/tests/import.rs`:

* `+0x258 == +0x268 − +0x26C − herdEaten` — one relation pinning three offsets.
* `+0x22C` is `Grain_LabourEstimate`'s tail. **Corpus limit, stated:** `+0x230`
  and `crop[2]` are zero in every county of every save, so the sowing and harvest
  arms are checked only at zero, and whether the tail's `season` is this season or
  the next cannot be told here. Five counties carry a non-zero change.
* `happinessAvg == (i8)(happinessSum / g_turnCount)`.
* `+0x5B` is non-zero exactly where `+0x2C ≥ 6`.
* `+0x21` is set only below happiness 30 — which identifies it as
  `Unrest_UpdateAll`'s warning latch, the flag `County::unrest_warned` said had no
  known offset.

**And what a green run did not measure.** 58 of the 177 claimed rows are zero in
every save — no castle under construction, no crop in the ground, no random event,
no alliance anywhere in the corpus — so for those the value check compared an
offset only with zero, and a wrong offset landing on another zero passes. The test
prints them by name every run rather than letting *30,600 agree* imply more.

**Excluded, and a player can see it: features, not imports.** Nine rows are drawn by
a painter in the original and have no field in `l2_kingdom` to carry them into:
`Panel_Ration`'s three requirement figures (`+0x16C`), `Panel_JobGrain`'s `+0x24C`
and `+0x278`, `Panel_JobCattle`'s `+0x270` pair, `Panel_JobIndustry`'s four
(`+0x280`), `Castle_BuildEstimate`'s `+0x1A6`, `Court_Draw`'s realm `+0x15C`, and
`Army_PickName`'s counters at realm `+0x2D`. **`+0x1AD`, the mercenary offer, is
excluded for a different reason and it is a finding:** it is a cache of
`g_mercenaryBands`, which no importer reads at all, so carrying the byte alone would
mark a band nobody can hire. A loaded game has no mercenary bands.

**Three things the tool found about our own documents.** `docs/records.json` typed
the three event swing bytes `u8`; `Event_RollAll` writes `0xD8` and `0xE2`, −40 and
−30, and they are now `i8`. The decompiled corpus still prints the pre-C153
`Industry` layout, so `fields.js` detects which layout a corpus was decompiled under
(only the new one names `nextSeason`) — without that, the county's `weaponType` byte,
read as `*(byte *)g_counties[x].industry`, resolved onto `efficiency` and +0x290 looked
untouched. And the first `--check` reported all 320 offsets uncovered because the rows
carry a `type` and the scanner read a `width` — the tool's own first bug, caught by its
own implausible number.

**Not moved into `l2-formats`.** The brief allowed it; the check does not need it,
because it decodes raw offsets itself, so `l2_formats::save::{County, Realm}` are
unchanged and the new reads sit in `l2-scenario` beside the ones C142 and C149 added.

---

**C162 — a save test failed one run in ninety, and the shared directory
it raced over was also hiding a real defect and three assertions that could not fail.**

`crates/l2-game/tests/save.rs`'s `the_save_screen_writes_a_file_and_the_load_screen_reads_it_back`
failed at merge twice in one evening and passed every time it was run alone. It was recorded as a
pre-existing flake in the merge of C151 and left there. This entry is what measuring it found.

**The flake, measured.** It failed **23 times in 2,000 runs** of the unfixed test binary, with
8 parallel workers — about 1.15 % — and always on *"the loaded game is the saved one"*. It was
then forced deterministically before anything was fixed. Barriers held two real tests at the
interleaving:
1. `a_failed_write_cannot_destroy_the_save_it_was_replacing` writes its file;
2. the load screen opens;
3. the file is removed;
4. the test takes its second listing.

Under that interleaving it failed 5 of 5 unfixed and passed 5 of 5 fixed, and the fixed binary
failed 0 of 2,000. **[V]**

**Two causes, both fixed.**

* **The test derived the row it clicked from a second `saves::list()`**, not from the list the
  screen opened with. Anything written or removed between the two reads moved the click one row,
  onto a different game. It now clicks a literal row, pinned by the files its own directory is
  asserted to hold.
* **Every file test shared one `LORDS2_SAVES` directory.** Two took a listing lock and four wrote
  or deleted without it. `saves::scoped_dir()` now gives each test a directory of its own, ahead
  of `LORDS2_SAVES`, and **the lock is deleted**. The result is a shape that cannot race, rather
  than a lock that the next test must remember to take. Making the scope process-global again
  turns the isolation test red on its own, and 8 of the file's 18 tests red.

**The near miss.** Before this, **a test that forgot to set up its directory would have written
straight into the player's real `%APPDATA%` saves.** `LORDS2_SAVES` now points underneath a
regular file. A test with no directory of its own therefore fails on its first write, naming the
path, whatever order the tests run in. Removing the directory from the screen test and running it
alone does exactly that, and leaves `%APPDATA%` untouched.

**What the shared directory was hiding.** Each item below was ablated red.

* **A real defect.** The save screen drew its directory path uncut, so a path over 67 characters
  overflowed the box, 398 pixels outside it. That takes a deep `LORDS2_SAVES`, or a profile user
  name over 24 characters. The test that it paints inside its window had passed only because its
  temp path was short. The line is now cut from the left.
* **Three tests that could never fail**, each shown green with the behaviour it claimed deleted:
  * **The overwrite test** looked for a `.part` file through `saves::list()`, which only shows
    `.l2sav`, so it could never see `x.l2sav.part`. It stayed green with the rename swapped for a
    copy.
  * **The cancel test** typed with key-down events the name field ignores, and searched for
    `"CANCELLED"`, which the field lower-cases. It stayed green with the cross wired to confirm.
  * **The sort test's** three lower-case names were already in order as NTFS returns them. It
    stayed green with the sort deleted; one capital letter now makes the two orders disagree.

**Production does not race.** The load screen reads the directory once, and both its drawing and
its click resolution use that one read. That is now pinned by a test: a save arriving after the
screen opens does not move the clicked row. The test goes red when the click arm re-reads the
directory.

**That is C138's shape three times in one file:** an assertion that cannot fail looks exactly like
one that passes. A flaky test was the only thing that made anybody read this file closely enough
to find them. "It fails one run in ninety" was the visible symptom of a directory shared by tests
that were not all testing what they said.

---

**C163 — `Font_10.pl8` is loaded, it has no letters, and the nine numbers a player reads every turn are in it.**

C157 left `g_font10` unloaded because pointing a test at it made a lowercase
letter draw nothing, and read that as **[I]** *"not an alphabet under the shared
table"*. Half of that was right. **[V]** throughout, from the decompilation and
the user's own files.

### `Glyph_Draw` has one table and no gap handling of its own

* `Glyph_Draw` (`0x00402A14`) reads `g_glyphWidths[c - 0x20] - 1` and the record
  at `font + frame * 0x10 + 8` for **every** face. It has no per-face table and
  no check against the file's frame count. Its only per-face branch is a
  one-pixel raise for `&g_fontBody` on characters `0x81…0x8D`, `0x93…0x97` and
  `0xA0…0xA4`, which `shell::font` does **not** reproduce.
* A gap is a zero entry. `Ui_DrawText` (`0x00402637`) advances 4 and does not
  call `Glyph_Draw` at all. Nothing substitutes a character or falls back to
  another face.
* The blit, `0x004B41B7`, is a mask in the caller's colour.
* Every table entry lands inside all five shipped faces. The highest frame
  asked for is 104, and the faces hold 150, 108, 108, 108 and 106.

So `Font_10.pl8` is read exactly as the other four are: same base, same indices.

### What the file holds

108 frames, the same layout as `Fntl2_9.pl8` and `Fntl2_14.pl8`.

* Frames 52…61 are the digits and 62…78 the punctuation strip. These are real
  ten-row glyphs.
* **All 52 letters and the 29-frame accented tail are 2 × 2 stubs.** Of the 81,
  61 are wholly transparent and 20 carry one to four stray pixels (`'e'` is a
  solid block).
* A letter blits its stub and advances 3. *"Seasons"* paints four pixels and
  moves the pen 21. That is the "renders nothing" of C157's panic.
* **[I]** The atlas positions in the stubs' records match a full alphabet's, so
  the letters were cropped out of a sheet that had them.

**Every string the nine call sites build is a lead, digits and one space.** The
leads are `' '`, `'@'`, `'+'` and `'-'`. The prefix and suffix pointers
`0x004D3D40…0x004D3D84` are all `" "`, read out of the image. The original
never asks this face for a letter. The words beside its numbers are
`&g_fontSmall`.

### All nine are drawn under `g_dropShadow`

Each of the eight row painters sets `g_dropShadow = 1` on entry. By then
`CountyStrip_Draw` has already cleared `DAT_005AEA40`. So `Ui_DrawText` takes
its drop-shadow arm: `(x + 1, y + 1)` in `0x3F`, then the glyph.
`font::DROP_SHADOW_COLOUR` had recorded that arm as unimplemented.
`Font::draw_dropped` now draws it. The castle cell's two `&g_fontSmall` captions
are inside the same flag and are dropped too.

### What changed

| draw | was | is |
|---|---|---|
| 7 × `Ui_DrawDelta` forecasts (`strip_delta`) — three farm rows, four industry rows | `Fntl2_9.pl8`, flat | `Font_10.pl8`, dropped |
| reclamation figure, `FUN_004103C5` | `Fntl2_9.pl8`, flat | `Font_10.pl8`, dropped |
| castle seasons, `CountyStrip_DrawCastleIcon` | `"{n} "`, **no lead**, `Fntl2_9.pl8`, flat | `' '`, `" "`: **digits +4**, `Font_10.pl8`, dropped |
| castle *"Season(s)"* / *"Needed"* | `Fntl2_9.pl8`, flat | `Fntl2_9.pl8`, dropped; plural rule is `Ui_DrawUnitNoun`'s `value == 1` |

`ten_text` carries a `debug_assert!` against letters, so a word routed to the
numeral face panics in tests. That assertion has **not** been observed firing.
`ShellAssets::ten` is new, and `missing_fonts` names all five faces.

**The strip has seven `Ui_DrawDelta` calls, not eight.** The seven are
`00410000.c` lines 22, 52, 78, 98, 114, 130 and 154: three farm rows and four
industry rows. The castle painter has none, and 7 + 2 `Ui_DrawNumber` = the
nine `&g_font10` references. `strip_delta`'s doc, `DELTA_POS`,
`draw_produce_rows` and the cattle test said eight, and are corrected.
`docs/draws-map.md` §5.5 (*"the eight `Ui_DrawDelta` calls"*, *"all eight rows
pass `mode = 0`"*) is not edited here.

### Tests, each ablated

| test | ablation | red with |
|---|---|---|
| `shell::font_10_is_a_numeral_face_read_through_the_shared_table` | file → `font::SMALL` | `'a'` 8 rows, expected 2 |
| 〃 | file → `font::EIGHT` | 150 frames, expected 108 |
| `screens::the_castle_cell_puts_its_number_in_font_10_and_its_word_in_fntl2_9` | no `' '` lead | digits at (572, 320), expected (576, 320) |
| 〃 | `small_dropped` → `shell.ten` | *"Seasons" is not on the castle cell* |
| 〃, `screens::the_industry_forecast_is_a_dropped_font_10_number` | shadow blit deleted | both shadow claims |
| the two above, plus `the_cattle_row…`, `the_grain_row…`, `the_reclamation_row…` and `a_loaded_game_draws_each_industry_rows_own_forecast…` | `ten_text` → `shell.small` | all six, at their `Font_10.pl8` search |

**Not separately observed red:**

* the castle word's ink count, which its glyph search implies;
* the word's shadow claim, which sits behind the number's;
* the *"Seasons"* ink bound in the shell test.

### `Ui_DrawNumber` has 190 call sites, not 191

`docs/plan.md` said 191. That was a text count: 191 lines of the decompilation
match `Ui_DrawNumber(`, and one of them is the definition,
`void __cdecl Ui_DrawNumber(`. The `====` header does not match. So there are
190 calls, which agrees with C157's 190 live and 211 in the exe.

`docs/plan.md` is corrected. The same 191 is still quoted in:

* `docs/draws-map.md` §5a (twice, and in its 351 total);
* C127 and C140 above;
* `Ui_DrawNumberRight`'s `symbols.json` comment.

Those are not edited here. §5a's lead breakdown, 115 + 62 + 1 + 1 = 179, does
not sum to either figure, and that is not explained here.

**Corrected at merge.** All four quotes above now say 190 and cite this entry: `docs/draws-map.md`
§5a (where the four routines' total becomes 350), C127, C140 and `Ui_DrawNumberRight`'s
`symbols.json` comment.

---

**C164 — no loaded game and no new game had a single mercenary
band, and thirteen of C161' 64 exclusions are things a player can
see.**

C161 left county `+0x1AD` excluded as *"a cache of
`g_mercenaryBands`, which no importer reads"*. Following it went further than an
import gap: **`MercenaryBands::init` was called by nothing but tests**, so a new
campaign had no bands either. `Game_NewGame` runs `Mercenary_Init` between
`Merchant_SpawnAll` and `PlayerStart_Shuffle`; our new-game path had no line for
it. On every game, loaded or new, the raise-army screen never offered a band and
C150's marker on the town tile never had one to mark.

**The table, `[V]` three ways, none of them resemblance to the roster.**
`g_mercBands` (`0x00568DC0`) is saved as a block of exactly **260** bytes, 13 ×
`0x14`: slot 0 and twelve bands. The six constant fields `Mercenary_Init` copies
equal the roster in all **72 bands over 18 saves**, and every slot past
`g_mercBandsInPlay` (`0x00554030`, also saved) is zero. And the walk checks
itself on data the original wrote: county `+0x1AD` is `Mercenary_OfferInCounty`
over the table in every county of every save — `siege-old_turn.sav` stands bands 2
and 3 in county 1 and the byte is **2** — every offering band has just reloaded its
countdown and stepped one past its county, and **one season of
`Mercenary_AdvanceAll` over each of the four one-turn pairs lands on the next
save's table exactly.** `england-turn1.sav` is `Mercenary_Init(14)` band for band.
The hirer word is zero in every band of every save; its check is exercised only by
a test that writes one.

**A second defect behind the first.** `Kingdom::start_new_game` walks the whole
pipeline, which carries the phase-7 walk; `Game_NewGame` is `Mercenary_Init(); …
Season_Advance();` with no `Mercenary_AdvanceAll`. Harmless while no new game had
bands, it would have had the Saxon band offering itself on turn one. It skips that
pass now, and a test compares a map-built England after its opening season with
the save. `Units_ResetMoves` and `Diplo_ReconcileAlliances` also run there and are
not in `Game_NewGame` either `[D]`; both look like no-ops on a new game and were
left alone.

**The 64, classified against the decompilation rather than their reasons.**
33 ignorable as stated. 13 ignorable with a wrong or incomplete reason, each
rewritten — among them realm `+0x13C`, whose "armoury wall and levy" readers index
`+0x13C + t*4` from `t = 1` and so read `+0x140` onward, a scanner artefact of
`fields.js`; and `+0x18C`/`+0x190`, which are not copies of `+0x178`/`+0x17C` and
differ from them in twelve and eighteen counties. **Five are rule inputs a loaded
game loses, and each needs a mechanic we have not built:** `+0x15A`
(`County_EnsurePasture` when cattle are bought), `+0x15B` (the field drought and
flooding ruin), `+0x206` (`County_DestroyField`'s separate sown count), `+0x29C`
(a trample does not reset the efficiency ramp's memory), realm `+0x2A` (the
conquest letters). **Thirteen are player-visible.**

**What moved.** Imported: `+0x1AD`; realm `+0x2D`, `Army_PickName`'s counters,
modelled and encoded all along and never read, so a loaded game named its next
army from a clean slate; and four new county fields at our `VERSION` 20 — what
last season's weather and event did to the grain (`+0x24C`, `+0x278`) and the herd
(`+0x270`, `+0x274`). Derived, each a function the code already had or now has:
the ration panel's *Fed* row (`+0x16C`), the castle estimate (`+0x1A6`), the four
industry figures (`+0x280`, `industry::panel_figures` — whose module docs said
*"no draw call reads them"*, and `Panel_JobIndustry` draws all four) and the
court's expected tax (realm `+0x15C`, now `Kingdom::tax_expected`). **242 rows:
173 imported, 14 derived, 55 excluded.**

**What a green run does not measure here.** Of the new claims, `+0x1A6`, `+0x24C`,
`+0x270`, `+0x274` and `+0x278` are zero in every save — every county on this
machine stands in *Cloudy* with no event live and no castle mid-build — so 63 of
the 187 claimed rows are now compared only with zero, five of them these. The
blacksmith pair of `+0x280` (sixteen and seven non-zero), realm `+0x15C` (23) and
realm `+0x2D` are measured against real numbers.

**Carried and still not drawn.** The grain, cattle and industry job popups' bodies
are stubs; `Castle_DrawStatusBlock` has no painter; the eight county-event letters
print no count. The figures are now there for those painters to read. County
`+0x2F8`, the Plague and Wedding letters' figure, stays excluded for a stronger
reason than a missing painter: `Population_UpdateAll` computes the event swing as
`Pct(deaths or births, |p|) + 10`, capped at a fifth of the population, and
`l2_kingdom::population` computes `Pct(population, p)` — a rule that differs from
the original's `[D]`, and carrying the byte would print a number the rule did not
apply.

---

**C165 — The options panels acted on the click on all twelve rows, and three of
the rows change nothing.**

The tip-screens branch found it while wiring `Opt_ToggleTipScreens`: every `Opt_Toggle*`
record is `Widget_Test` **kind 5**, and our options screen toggled on the press. That is the
yes/no gauntlets' defect (C148) on a screen C148 could not reach, because `options.rs` was
one of the modules `docs/arms.json`'s own note named as carrying no marker — so `arms.rs`,
which checks markers against records, had nothing to check.

**Enumerated from both sides**, as the `army-division` group was: `Screen_HandleInput`'s four
`Widget_Test` calls (twelve records, kind 5, frame 25) and `Screen_FrameInput`'s four arms (a
right release and `Ui_OkButtonClicked` on each, and 0x39's sync latch). **Eighteen records**
in a new group `options-panels`: twelve rows (eleven `reproduced`, *Start game help*
`missing`), the OK corner and the right release (`reproduced`), the latch (`missing`), our
Escape and our quirks page (`invention`, kept), and the orphaned kind-4 table at
`0x004DDE08` (`dead`).

**What reading the twelve handlers against their readers found**, which is the part a kind
fix alone would have missed:

| row | what was wrong |
|---|---|
| all twelve | acted on the press; now kind 5 through `press::Press`, pressed frame `25 + 1` |
| the OK corner | closed on the press; `Ui_OkButtonClicked` is the release |
| widget picture | drew `Ui_OkButton` mode 1 (frame `0x10`), which is no record's frame; the records say 25 |
| Army foraging | flipped the flag and **re-ran nothing**; `Opt_ToggleArmyForaging` re-runs `Ration_Apply` and `County_RefreshEstimates` over every county |
| Full screen | did nothing, not even close; on any non-8bpp desktop the original closes the panel and posts message `0x104`, *"Cannot change display."* |
| Exploration, Animations, Tool tips | **flip a field nothing in this engine reads** — the fog, five animation readers in four functions, and the tooltip layer `FUN_00476E95` are all unbuilt |

**And one documented claim was false**: `docs/symbols.json` and `docs/screens-county.md`
§10.2 said the four advanced handlers *"show tip 0x32"* in a network game. It is
`Net_SendCommand(0x32, 0)` — net opcode `0x32`, whose writer sends the row number. Corrected
in `screens-county.md`; `symbols.json` is the lead's.

**A limit of the check, measured by ablation**: declaring every row `Kind::Press` leaves
`arms.rs` green. It compares the marker's word with the record's and the record's with the
exe, and never reads the `Kind` the code declares — the marker is text. What went red was
`tests/options.rs`. The exe-gated check also had to learn one thing: `Opt_ToggleSpeech` and
`Opt_ToggleAnimations` are each kind 5 in their panel and kind 4 in the orphaned table, so
their address alone is ambiguous; a record whose prose names one record base calling that
handler is now judged by that record. Coverage 37 → 54 by handler, 12 → 31 by prose.

---

**C166 — battles were nearly silent because the simulation kept no record of a
swing, a hit, a loose or a death, and "a limit of the design" did not follow from anything.**

The battlefield's per-man sound sites had been filed `blocked` on *"per-man events"*, and
`docs/audio-triggers.md` called that *"a limit of the design"*. **The gap was the event stream, not
the sounds.** `l2-sim` resolved figure and unit state and recorded nothing a listener could hear,
so there was nothing for a sound to answer. Nothing in the design required that; it was a record
nobody had written.

**The record.** `crates/l2-sim/src/cue.rs` holds monotone counts of the occasions the original's
per-man code sounds on:
* a man falling, by the striker's troop;
* a figure's last man, by side;
* a missile loosed, hitting, felling and killing, by weapon;
* a wall shot, and a wall smashed.

The battle writes these counters and **never reads them**, and they are **excluded from the
lockstep checksum on purpose**.

**Why counters are exact rather than an approximation.** Every battlefield sound site goes through
`Sound_PlaySlot`, its thunk `FUN_004262cf`, or `Sound_PlayFile`. All three **drop a request while
their buffer is still playing** `[V]`, and that drop is the original's only throttle. So *"did
this happen since the last tick"* is all the original could ever make audible, and a count answers
exactly that question. We add no throttle of our own.

**22 of the 31 battlefield sites are now wired**, including all six troop-cry sites. The nine left
are each blocked on a mechanic `l2-sim` lacks — fire, boiling oil, tower docking, state 17's
loose, the high-rampart catapult miss, a realm eliminated mid-battle — and `docs/audio.json` names
each one.

**Troop cries use no random number at all.** `Sound_PlayTroopCry` is a round robin per
(troop, order type) over `g_troopSounds` (`0x004DB0D0`), an 11 × 4 × 4 table transcribed and
asserted against the executable. The index is stepped before it is read and never reset between
battles, and the take is spent even when the cry is dropped by the busy buffer. With no RNG, there
is nothing to keep away from the battle's own `Pcg32`.

**Determinism, proven rather than argued** `[V]`:
* A battle with a sound `Director` listening and one without are **identical every tick for 4,000
  ticks**, and **byte-identical when saved**. That holds with silent audio and with real decoded
  audio.
* A battle whose cue record is **wiped every tick** equals the untouched one. Nothing reads the
  record back into the simulation.

Each of those tests was ablated and seen red.

**Counts** (from `docs/audio.json` on the branch's base): sound triggers reproduced went from 51 to
73 of 143, and reachable files from 560 to 639 of 771. `docs/bugs.md` D34 is corrected: nine
unreachable cry names ship in neither install, not seven.

**The lesson is C140's and C139's.** A blocker's stated reason was a claim, and nobody had checked
it. *"A limit of the design"* read as a finding and stopped the work for as long as it stood. The
design asked only for a record the simulation never reads, which is the cheapest kind of state
lockstep has.

**At merge, with the tip screens already on `main`:** 75 of 143 reproduced and 674 of 771 files
reachable, from the merged `docs/audio.json` and the tests. Both branches had added a record of
the one-shot buffer, `DAT_00522AEC`: the tips' `last_speech` for `Sound_OneShotBusy` and this
branch's `one_shot` for `Sound_PlayFile`'s drop. The original has one buffer and both ask it,
so they are one field, set by `play_speech` and `play_file` and read by both checks (since C176, `stop_and_play_file` in `play_speech`'s place). A troop
cry now keeps the tips' chained takes waiting, as it would in the original.

---

**C167 — the tool tips are built, and the sentence that found them
counted twenty-six as twenty-four and called a ladder's answer a hotspot id.**

The Help Options panel's *"Tool tips"* row flipped `g_optToolTips` and nothing here
read it. `FUN_00476E95` is now `crates/l2-game/src/tooltip.rs`, and reading all seven
of its functions — not the one C86 summarised — corrected two things on file and
found three that were not.

**Corrected.** C86, `docs/screens.md` §7 and `docs/draws-map.md` §5.1 said *"twenty-four
of the thirty-five strings are the campaign sidebar"*. `FUN_00477320` returns 1…22 and
31…34: **twenty-six**, and the listing printed directly under the sentence in
`draws-map.md` has all twenty-six in it. And `docs/formats/eng.md` §5 said *"index =
hotspot id"*, which reads as `g_uiHotspotId`, the widget record's `+0x10`. Neither
resolver reads a widget record: the id is a pointer ladder's own answer, and the ladder
reads live state — the minimap mode, whether the selected county is the player's, and
`FUN_0040FEC1`'s two produce-row lists.

**Not on file.** `[V]`, each asserted in `crates/l2-game/tests/tooltips.rs`:

* **The lookup is a table of screens, not of controls.** `DAT_004D6FB8[g_screenId]`
  gives the sidebar's ladder to **thirty-five** screen ids and the battlefield's
  (`FUN_004777AA`, eight ids) to `0x29` alone. So the sidebar's tips go on showing over
  the county panels, the village, the job popup, the options pages and an open menu, and
  never over the merchant, the armoury, the other lords or a tip screen (`0x27`).
* **The rest is wall clock, not frames**: `999 < timeGetTime() - stamp`, which is 63 of
  our ticks. The briefs that asked for *"the frame count"* were reasonable — the tip
  screens and the options both turned on twenty-frame countdowns — and this one is not
  one.
* **The frame that hides a tip writes no stamp.** A tip up for a second and nudged for
  one frame is back on the next still frame; a nudge of a fresh tip waits a second from
  the tip's own resolve, not from the nudge. `Opt_ToggleToolTips` and `Map_InitMode`
  both zero the stamp, so turning the option on shows a tip with no rest at all.

---

**C168 — The AI farmed once a turn and shopped never, and the county that "began short of food" was fed on cheese.**

C149 named three pieces of the AI's farming it did not port. All three are ported now, each
read out of `Lords2.exe` at the moment of writing rather than out of the entry that named it.

**1. `Ai_ManageFarmsAll` (`0x0049A990`) is `Season_Advance`'s first call.** `0x00448440`
opens `Ai_ManageFarmsAll(); Rand_Advance();` and only then reads the clock, and the function
is `Ai_ManageCountyFarms` (`0x0049DD01`) for every realm with `strength != 0 && isHuman == 0`.
So an AI lord's counties are farmed **twice a turn** — his own step 5 in phase 4, and again
ahead of tax, rations and industry, on the season that is ending. `Pass::AiManageFarms` at
position 0 of `SEASON_PIPELINE`. `[V]`.

**The class is four callers of `Ai_ManageCountyFarms`, not two, and one is not ported.**
`FUN_0049DF48` is the same loop with its two tests swapped, and its only caller is the tail of
`Battle_ReturnToCampaign` (`0x004AB383`): `FUN_004AD426(); Panels_RefreshAll(); FUN_0049DF48();`.
`l2_kingdom::battle::return_to_campaign` carries none of that tail, so here an AI's farms are
**not** re-managed after a battle. Open.

**2. The stall's owned-county arm.** `Ai_BuyGood` (`0x004A4B12`) tests
`price * qty <= g_realms[owner].gold` for an owned county, and `Merchant_Trade` (`0x004284CE`)
then writes `gold -= bill; tradeSpentB += bill; tradeSpentA += bill`. `CountyStall` now carries
the realms and does both, and AI step 5 and the season head pass it instead of `NoMarket`. `[V]`.
**Not ported, and it is the nearest remaining gap:** each realm style opens with
`Ai_TradeForCounty` (`0x0049E39B`) — the surplus sale and the weapon purchase — before its
`Ai_BuyGood` lines.

**3. Realm `+0xF4`/`+0xF8`, and what they are for: nothing a rule reads.** Writers:
`Tax_CollectAll` (`0x0044B59B`) credits both with every owned county's take, beside the
treasury; `Game_SetupRealmsAndCounties` (`0x0049BD99`) clears them. Readers, by two checks that
share no step: the decompilation names `field_0xf4`/`field_0xf8` of `g_realms` in those two
functions only; and a scan of `Lords2.exe` for the absolute addresses `0x0057BFF4` and
`0x0057BFF8` finds four instructions — `0x0044B7BD`, `0x0044B7DE`, `0x0049C364`, `0x0049C37C` —
all inside those two. The same scan finds the trade pair's `+0x10C` only in `Merchant_Trade` and
the clear, and the score's `+0x50` fourteen times, so it sees readers where there are readers.
It cannot see an access through a record pointer plus a small displacement. The only
whole-record consumers are `Save_Write` and `Sync_CompareState`, so the pair is **stored,
desync-checked and unread** — carried exactly as the trade pair is: `Realm::tax_ledger`,
`l2_kingdom::save::VERSION` 20 (+48 bytes), imported from a `.sav`, and its
`docs/stored-fields.json` row moved from `excluded` to `imported`.

**The differential, before and after.** `AGREE_TOTAL` 900 → **906** of 932, `MOVED_AGREE_TOTAL`
258 → **264** of 279. Six divergences went, all on realm 2 and none arrived: battle 3->4's
`realm.wood` (+31), `realm.iron` (+5) and `realm.weapons.4` (−2), and siege 12->13's
`realm.iron` (+35), `realm.wood` (+35) and `realm.weapons.1` (−6) — all outputs of
`Industry_ProduceAll`, which reads the labour split and industry share the season-head pass
resets. **Attributed by ablation**: emptying the `Pass::AiManageFarms` arm alone, with the stall
arm and the ledger still in, restores exactly 900 and 258 and all six rows. So on these four
pairs the stall and the ledger moved nothing. **Not moved:** realm 2's `realm.gold` (+5, and
+2,005 on siege 13->14), `realm.wages` and `realm.score`. On siege 13->14 the original's realm 2
spends about 1,620 crowns and gains a hundred maces, which is the shape of `Ai_TradeForCounty`.
`[I]`; not chased.

**The trap, in the place the brief said to look.** Wiring the pass turned
`realm_fives_county_diverges_because_the_save_does_not_record_what_it_ate` red. That test,
`docs/kingdom.md` §4.3 and `reproduction.rs`'s own module documentation said realm 5's county
must have opened on **eight sacks** the save did not record, and that this settled
`Ration_Apply` debiting the store. **The save records it, and there were no sacks.**
`Grain_SeasonTick` (`0x0044C8AE`) and `Herd_SeasonTick` (`0x0044D60D`) each open by copying the
store into county `+0x228` / `+0x254` and only then take the season's food out;
`Game_SetupRealmsAndCounties` writes the new-game stores into the same two fields. On the
England fixture `+0x254` is **95** in every county: the county went into the ration pass with
95 head and fed 417 people on cheese. Starting all fourteen counties from `+0x228`/`+0x254`
reproduces twenty-five fields each, the ration split included, with no inversion anywhere — a
test now. The eight sacks were the unique answer of a model that was ours twice over: a ration
pass that debits the store (C149 read `Ration_Apply` and found no `-=`) and that saw the herd
the season *ended* on. **Uniqueness inside a wrong model is not a measurement.**

**What that new test does not show, measured:** it passes with the pass ablated too, so it is
evidence about the rewind and none about the pass. What pins the pass on that fixture is the
rewritten realm-5 test: from the rewound stores (no grain, 74 head) the lord's own
`Ai_SetRations(county, 0)` (`0x004A4782` — an upward sweep that keeps the first strict
improvement, read and matching our port) settles on split 100 and feeds Normal on slaughter, so
happiness, health, deaths and population land on the file by a different road; ablate the pass
and the old Half comes back. `l2_scenario::Scenario::starting_kingdom` still rewinds to the
post-season stores, and says so at the function; switching it changes what a rewound position
*is* and is left to the importer's owner.

---

**C169 — A plague took its percentage of the county; the original takes it of
the season's deaths, adds ten, and prints the result.**

Found by the stored-fields classification, not by a player: county `+0x2F8` was excluded
because *our rule could not have produced it*, which is a rule difference wearing a data
field's clothes. `Population_UpdateAll` (`0x00449EF3`), after the `+1` that goes to whichever
of births and deaths the rates favour:

```c
county.+0x2F8 = 0;
if ((char)eventPct < 0)      county.+0x2F8 = Pct(deaths, -eventPct) + 10;
else if ((char)eventPct > 0) county.+0x2F8 = Pct(births,  eventPct) + 10;
if (Pct(pop, 20) < county.+0x2F8) county.+0x2F8 = Pct(pop, 20);
if (eventPct < 0) deaths += county.+0x2F8; else if (eventPct > 0) births += county.+0x2F8;
```

Ours was `clamp(Pct(population, pct), -Pct(population, 20), Pct(population, 20))`. **[V]**, and
closed three ways:

* **Every event through that code.** The corpus writes `+0x1FB` ten times: two clears
  (`Event_RollAll`, `Event_ClearCountyModifiers`), four season-picked writes in `FUN_00448F6F`
  — *Plague*, Winter −40, Spring −30, Summer −20, Autumn −30 — and four in `FUN_004491F0` —
  *Wedding fever*, Winter +30, Spring +60, Summer +50, Autumn +40. `Population_UpdateAll` is
  its only reader — `node tools/oracle/fields.js` agrees on the reader, and its table prints
  only the first three writers. Those percentages match `EventKind::effect` already; only the
  application was wrong. No other event moves people.
* **Every reader of the result.** `+0x2F8` is touched by exactly two functions:
  `Population_UpdateAll`, and `Msg_DrawWindow`, which draws it with
  `Ui_DrawNumber(+0x2F8, '@', " ", …)` for event `0x8A` before `L2.eng` group 77 index 29,
  *"extra deaths."*, and for `0x8E` before index 30, *"extra births."* The game's own label says
  the figure is an addition to the season's deaths or births, which is what the formula makes
  it. (`FUN_0045337B`'s `+0x2F8` is a sprite blitter's offset, not a county's.)
* **The base is the adjusted figure** — deaths after the `+2` for Diseased and the `+1`, births
  after their `+1` — because the swing is computed below those lines.

**How big the difference was.** A Winter plague on 1,000 people in health band 2 at happiness
50: ours killed **200** extra, the original **74** (`Pct(161, 40) + 10`). A Summer plague on a
Perfect-health county, which was going to lose nobody: ours 200, the original **10**. The cap
still bites on a small county — a Spring wedding on 100 people asks 46 and gets 20.

**Where it came from, which is the part worth keeping.** `docs/kingdom.md` §5's `[V]`
pseudocode carried the line as a comment — `/* random-event modifier, capped at 20 % of the
population */` — and §8.1's handler table said *"pop % by season"*. Both are true sentences
about the byte; both read as a percentage of the population; and the code built from them took
one. The CLAUDE.md warning about `[V]` documents producing defects, a fourth time.

**Not the original's bug in this reading.** The percentage-of-births reading is what group 142
says (*"a jump in the number of children born"*), and a cap that only bites below a few hundred
people is odd rather than broken. `docs/bugs.md` §4.2, **[I]** on "intended".

**What the fixtures could and could not settle.** No save on this machine carries a live
figure: `+0x2F8` is **zero in every county of all 18 saves**. They are not silent about
events — county 3 holds Wedding fever's id `0x8E` in `siege-safeturn`, `siege-old_turn`,
`siege-lastturn` and `siege-sieging` — but that id is **stale**: `Event_RollAll` clears the three
swing bytes and `+0x1A8` every season and never `eventId` or `eventFired` (only a failed guard
and `FUN_00448D7E`'s enqueue clear those), and the saved births reproduce with no swing in them.
So the rule is pinned by hand-worked numbers, and the import by
`crates/l2-scenario/tests/import.rs`'s `the_plague_letters_figure_survives_a_load`, which
patches the figure into a real save's bytes — **ablated: deleting the importer's assignment
turns it red and leaves `tests/stored_fields.rs` green**, because a row that is zero in every
save is compared with zero.

**Changed.** `l2_kingdom::population::update_one` is the code above, writing
`County::event_population_swing`; `+0x2F8` is imported (`docs/stored-fields.json`, `excluded` →
`imported`, renamed `eventPopulationSwing`); our save format carries it at `VERSION` 20.
**`crates/l2-game/tests/differential.rs` did not move** — 932 compared, 900 agree, 279 moved,
258 of those agree. No pair's original season ran a population event — `+0x2F8` is zero in all
four after-saves — and **[I]** neither did ours: the old and new rules disagree on every county
an event reaches, so an event in our run would have moved a births or deaths row.

**Two leads, not fixed.** *Ours clears `event_fired` and `event_id` every season*
(`l2_kingdom::event::roll_all`), and the original does not, so an unshown letter survives a
season there and not here. *Our message screen draws no figure*: `screens/message.rs` sends
category `0x0F` to `draw_notice`, and `Msg_DrawWindow`'s event arms draw a number under the body
for eight of the twenty-four — `+0x278` for *Rats* and *Grain found*, `+0x274` for the four herd
events, `+0x2F8` for these two — each followed by its group 77 words. A rule-6 gap.

---

**C170 — The season's extra person follows the birth rate after happiness has
scaled it; we compared the rate before, and the differential's one-person gap was that.**

Found while porting C169, eight lines above it in the same function.
`Population_UpdateAll` (`0x00449EF3`):

```c
iVar2   = Table_Lookup(pop, &g_birthRateLadder, 20, 1);       /* base            */
iVar3   = g_deathRateByHealth[band] + g_deathRateBySeason[season];
local_c = Pct(iVar2, factor);                                 /* the SCALED rate */
births  = Pct(pop, local_c);
if (births == 0 && local_c != 0) births = 1;
if (local_c < iVar3) deaths += 1; else births += 1;
```

`l2_kingdom::population::update_one` tested `base` in both places. `docs/kingdom.md` §5's
`[V]` pseudocode wrote `birthRate` — a name it never defined, one line below `births = Pct(pop,
Pct(base, factor))` — and `base` was the nearest thing called a birth rate. **[V]**: the
decompiled body, and `crates/l2-game/tests/differential.rs`, which had been printing the
consequence since it was written.

**What it was, in the original's own saves.** *"The population arithmetic comes out within one or
two of the original's — births 79 against 80, deaths 96 against 95"* was the differential's good
news, and it was a rule. `siege-lastturn.sav` county 1: 799 people, factor 75, ladder rate 14, so
the scaled rate is 10; Spring in band 2 is a death rate of 12. `10 < 12` puts the person in the
deaths, **79 and 96, which is the file**; `14 ≥ 12` put it in the births, 80 and 95, which was
ours. `siege-old_turn.sav` county 3 is the same at 11%. Both are now a unit test with the file's
numbers typed in.

**Moved, and ablated.** `differential.rs`: **agree 900 → 907, moved-and-agree 258 → 264** of 279;
seven rows gone — `births`, `deaths` and `population` on both siege pairs and `pop_band` on
12->13 — and none arrived. Putting `base < death` back restores all seven, and turns four tests
red: the new unit test, both B16 tests and `realm_fives_county_diverges_…`.

**It retracts part of C69, and the retraction is the reason to read this.** C69 found by an
exhaustive survey that a county's death count cannot go negative on the season it dies, only on
the season after, and the B16 test and `Quirk::ExtinctCountyRecordsNegativeDeaths`'s doc said
the same. The survey was exhaustive and correct **about our function**. A county of one sits on
the ladder's 100% rung, which beats every death rate, so our comparison always gave it the extra
birth and it landed on zero. The original scales that 100% by happiness first — 25% at no
happiness, below a Diseased Winter's 43% — so the extra person dies and the county records −2.
`docs/bugs.md` B16's plain sentence had been right all along. **A survey over our own
implementation is a statement about our implementation**, and it overturned a correct catalogue
entry — the same shape as C61 overturning C58, with a test to make it look settled. C69's
paragraph carries a pointer; `crates/l2-kingdom/tests/quirks.rs` now asserts the survey turned
round (a living county does go negative, and the fixed path never does).

**Also changed.** `realm_fives_county_diverges_…` in `tests/reproduction.rs` pinned our divergent
chain at 66 deaths and 414 people; it is 67 and 412 now, and its *"births match either way"* line
was the same error — band 2's 16% sends the person to the deaths and the file's band 3 to the
births, 62 against 63.

---

**C171 — The game's films play, from a decoder of ours; and five things the inventories said about them were wrong.**

`crates/l2-smk` decodes all 45 `.smk` files and `crates/l2-game` plays each where one of
`Smk_Play`'s seven callers does. `docs/formats/smk.md` is the whole of it; this entry is the
part that corrects something already on file.

### The premise of D5a is overtaken, not settled

D5a asked which LGPL decoder to link, on the finding that no permissive one exists. None is
linked: the decoder is ours, MIT, written from the format description. **What makes that safe
to believe is the method, and it is worth reusing:** the LGPL `smk` crate was built in the
scratchpad and run as a **black box** — its API read off generated rustdoc, its source never
opened, nothing committed — and its per-film hashes of every frame's pixels, palette and
samples are pinned as literals in `crates/l2-smk/tests/corpus.rs`. A copyleft implementation
can be an oracle without being a source. D5a itself is left for the lead to close.

### `Msg_DrawWindow#16` and `#21` do not speak after the film

`docs/audio.json` and `docs/audio-triggers.md` both said the animated capture and ending
branches *"speak AFTERWARDS … the trigger is the film ending"*. `Smk_Play` returns as soon as
`Smk_Open` has decoded the first frame and run `Smk_PlayLoop` once, so the
`Msg_PlayVoice(DAT_004F0374, DAT_004F0354)` on the next line runs **with the film's opening**.
**[V]** from `Smk_Open`'s body. Built the other way it would have been a voice nobody hears
until a film is over.

### The capture film cannot be reached, and two sound sites stay `blocked` for it

`County_ChangeOwner` (`0x004A72FE`) posts the nine *"we have taken"* letters, groups
`0x75`…`0x7E`, with **category `0x0D`** — the animated capture branch's category. Nothing in
this workspace posts one: `l2_kingdom::conquest::change_owner` says the letters are left to a
caller, and there is none. So the capture film, its window and its voice are built and tested
with a hand-posted record, and `Msg_DrawWindow#15` / `#16` are **not** marked reproduced — the
same call `#24` got for the tip screens. The name of the gap is the letters, not the film.

### Setup page 1's ids were never contradictory

`crates/l2-game/src/screens/setup.rs` recorded as unresolved that `FUN_00432B05`'s hotspot 4
plays `lom.smk` while the painter draws *"Lords of Magic?"* third. `node tools/oracle/widgets.js
widgets 4dcb48 4` answers it: the table's records are in drawing order and carry ids 1, 2, **4**,
3 — and the third is the page's only **kind 3** record, so the trailer fires on the release
while its neighbours fire on the press. The item had been wired to page 10.

### The outcome banner read four siege outcomes as two

`outcome_pair` mapped a siege to pair 2 when the player won and 3 when he lost.
`Battle_SelectOutcomeBanner` (`0x00478419`) has four: took the castle 2, **held it 4**, driven
off 3, **lost it 5** — so a player who held his castle was told he had taken one. `outcome_banner`
replaces it for both banner arms and picks the battle film's row. **[V]** from the body; the four
arms read as four different `L2.eng` group 82 sentences, which is the check on the mapping.

### Screen `0x44` has no writer

An image-wide scan for `C6 05 50 AC 4E 00 xx` — `mov byte ptr [g_screenId], imm8` — finds 52
distinct immediates and no `0x44`; the other 48 stores to `g_screenId` are `mov [g_screenId], al`
restoring a remembered screen. So the Smacker test page, `Smk_ReplayIntro` and the forty-name
table behind it are unreachable, and `docs/arms.json` files its two widget handlers `dead`.
**[D]**: exhaustive over immediates, and relies on the register stores only ever copying ids
that were written first.

---

**C172 — The Exploration option flipped a flag nothing read, and the fog is built now.**

`g_optExploration` (`0x0053F264`) has been in `Options`, the setup screen, the options page
and the save since save version 12, and **nothing in this engine read it**: the switch looked
like it worked and did not. In `Lords2.exe` it has eleven readers and every one but the
options page's Yes/No is a map painter. **[V]**, each read in the decompilation:
`Map_RenderIso` (`0x0040526E`, two surround arms), `Map_RenderAlignedRow` (`0x00405AE9`, one),
`Map_RenderOffsetRow` (`0x00405C2F`, three), `Map_DrawTile` (`0x004063C1`), `Map_DrawTileApex`
(`0x00406673`), `Sprite_TopIt` (`0x004071A0`), `Map_DrawArmies` (`0x00408438`) and the dead
`FUN_00406BBA`; `Screen_AdvancedOptions` (`0x00414F68`) draws it; `Opt_ToggleExploration`
(`0x00434693`) and `NetAct_SetGameOptions` (`0x00447E0B`) write it; `Setup_CommitOptions`
and two resets zero or commit it. **No input arm, no AI step, no simulation pass and not the
minimap reads the option or its bit** — the AI lords see everything.

The painters test it with tile record `+2` bit `0x20`, the seen bit. Its writers, all
**[V]**: `FUN_0046E067(x, y, r)` sets the `(2r+1)²` square; `Unit_Step` (at every tile
centre, army only) and `Army_Create` pass 6; `FUN_0046DFD5` passes 1 for every tile of a
county and is called by `County_ChangeOwner` and at the end of
`Game_SetupRealmsAndCounties`; `FUN_0046DF51` clears the plane from `Map_InitScenario` and is
the only thing that ever clears a bit. **Every writer tests `g_localPlayer` and none tests the
option.** `Save_Write` carries the plane, because `g_tiles` is block 0.

**Checked against data, not only code.** The England turn-one fixture was saved with the
option off, and its seen bits are *exactly* the local player's county and a one-tile border —
bit for bit, 4,096 tiles (`crates/l2-scenario/tests/explored.rs`). The battle and turn saves
hold at least the squares round their armies and the borders of their counties; what else
they hold is where those armies walked, which no save records, so that half is a lower bound
and is stated as one. **The radius of 6 is verified from below by data and exactly only by
the decompiled literal.**

**Three things this found that were wrong about our own documents.** `draws-map.md` §5.2
said *"six draw functions"* and listed seven, named the toggle as `FUN_00447E0B` (that is the
network action), and gave `FUN_0046E067` a width and height it does not take.
`docs/mechanics.md` said the work was *"setting it as units move and counties change hands"*,
and it is four writers, not two: the new game's start county and a raised army both reveal.
And a draw test's own first version asserted that the `base` bank's frame 0 — the picture of
the dark — was blank at both zooms. It is blank at the near zoom and a green diamond at the
far one; the test's option-off control caught the first half of that and a measurement
settled the second.

**The one divergence is representation.** The original's plane is the local player's, so a
network game's machines each hold a different one — which cannot be lockstep state. Ours
keeps a bit per realm (`l2_kingdom::explore::Explored`), every writer sets the bit of the
realm the original sets it for *when that realm is the local player*, and the viewer's plane
is exactly the original's. It is simulation-written, in the save (version 20) and in the
digest. **And one ordering, disclosed:** our walker marks an army stopped on the commit that
empties its path, where the original stops after crossing into the last tile, so the
destination's square is revealed on the commit — same tiles, one crossing earlier. *(Gone:
C184 stops the walker where the original does, and the reveal is at the edge again.)*

---

**C173 — A left click on a field opens the information panel, and the panel says what the field is.**

Three reports from one player, whose reports of the original have held without exception:
the left click on a field *"still brings up placeholder"*, the right click opens the right
screen with *"the text for that field … not filled in"*, and there are *"debug squares still
on the town square … and the fields"*. All three **[V]** against the decompilation.

**The click.** `Map_Click` (`0x0043CE1A`)'s farmland arm is `_DAT_005681CC = 3; g_screenId =
4; FUN_0041B032();`, and the right release's `FUN_0043CAF4` ends in the same two statements;
`FUN_0041B032` picks the tile half on `g_pickedTileUnit == 0`. So on your own field the two
buttons open one screen. `docs/arms.json` `0x0043CE1A/field-brush` said the mode byte *"is
what makes the information panel draw the crop table instead of the tile panel"* — nothing on
that path reads it, and both buttons write the same 3 — and the arm was answered with a popup
of ours on screen 0, `ours/brush-popup-on-the-map`, now removed. The paths still differ in
one respect, recorded and not built: `FUN_0043CAF4` also selects and recentres on another
county when no unit is picked.

**The words.** `TileInfo_Draw` (`0x0041C208`) draws a field from `DAT_004D2EC8`, sixteen
bytes a terrain value — heading, body, icon, mode — and **the table's own descriptions for
wheat (30/35 … 30/39) and cattle (30/44 … 30/47) are read and never drawn**: those two modes
run `TileInfo_DrawGrain` and `TileInfo_DrawHerd` instead, which draw groups 77 and 22. The
table is transcribed and asserted against the player's image. `FUN_0041BEFE` also draws an
inset well we did not, and gives a flooded or parched field of yours row `0x11`, not 5.

**What is still missing, measured.** Four county figures those reports draw are excluded in
`docs/stored-fields.json`: `+0x278` and `+0x274`, what an event did to the grain and the herd,
and `+0x24C` and `+0x270`, what the weather did. The *"no outside factors"* sentences are
drawn exactly when the original's figure is provably zero — `Event_RollAll` runs before the
grain tick and only *Rats*, *Grain found*, *Mad cows*, *Wolves*, *Bad cattle* and *Cow
bonanza* write the two percentage bytes — and the figure lines are not drawn otherwise.

**The squares, and every other thing of ours on the map and the screens**, are behind a
debug overlay, off by default, flipped by Ctrl+D (`Prefs::debug_overlay`,
`ours/debug-overlay-toggle`): `Sprite_TopIt` puts a banner on the town's quadrant 0 and a
herd on a pasture and nothing else, and `Sidebar_ButtonClicked` is a hit test that draws
nothing. Ctrl+D because the window procedure has no letter arm and `Edit_TypeChar` rejects the
`0x04` it sends.

---

**C174 — The information panel's body-face headings were in the unit half, not the tile half: three drawn in body and five not drawn.** **[V]**

C150 recorded that *"the tile half's headings are `&g_fontHeading`"* and *"the unit half's use
`Pen::eng`, which is the body font, at the same slot"*, and left *"five call sites in a half
this change does not touch"*. The brief that followed read that as five call sites in
`TileInfo_Draw`. Reading both painters' call sites:

| painter | `&g_fontHeading` calls | ours before |
|---|---:|---|
| `TileInfo_Draw` (`0x0041C208`) | 3 — heading 30/`local_20` at `(0x28, R*16+0x40)`, its farm-field suffix 30/`local_c` at `g_penAdvance + 0x28`, the county name `Ui_DrawCentred(100, …, 8, R*16+0x18, 0x1C0)` | the county-town heading and the county name, both already heading; the suffix belongs to the farmland arm, which is not built |
| `TileInfo_DrawGrain`, `…Herd`, `…Castle` | 0 | — |
| `UnitPanel_Draw` (`0x0041B19D`) | 8 | below |

`UnitPanel_Draw`'s eight, and what each draws now:

| call | was | is |
|---|---|---|
| 31/2 at `(0x18, R*16+0x30)`, transport | body, at `(0x28, R*16+0x40)` | heading, at `(0x18, R*16+0x30)` |
| 100/`unit[+0x167] + scen*20` at `g_penAdvance + 0x18`, transport | not drawn | heading — the cargo county |
| 31/`local_20` at `(0x28, R*16+0x40)`, merchant 0 and peasants 5 | body | heading |
| `(owner + 0x5D)`/`nameIndex` at `(0x28, R*16+0x30)`, every army | not drawn | heading |
| 16/0 at `(0x38, R*16+0x130)`, own army with no band | not drawn | heading |
| `Ui_DrawNumber(mercMen, '@', "", 0x38, …)`, 16/`mercBand`, `Ui_DrawUnitNoun(mercMen, mercTroop*2 + 0x34, …)` | not drawn | heading, chained on `g_penAdvance` |

`DAT_004D422C` is a NUL, read out of the image. `Ui_DrawUnitNoun` (`0x0041AC3E`) is
`value == 1 ? index : index + 1`, without `Ui_DrawCount`'s `-1` arm. C150's *"five"* is
`UnitPanel_Draw`'s `Eng_DrawString` heading calls outside the mercenary-band branch.

**And a claim in `docs/draws.md` is contradicted by the same function.** Its *And a second
dead thing* says `local_14` is *"read nowhere in the function"*;
`Sprite_WGenSprite(local_14, 0x28, R*16+0x60)` reads it, and `screens/info.rs` already draws
those five frames as the unit icons. Not edited here.

Test: `chrome_text::every_unit_panel_heading_is_in_the_heading_face_inside_its_box` — six
units, each line found in `Fntl2_22.pl8` at its call site's own position, inside
`Ui_DrawBox(8, R*0x10 + 0x20, 0x1C, 0x1B − R)`, and not found in `Fntl2_14.pl8`.

---

**C175 — `Glyph_Draw` raises 23 characters in the body face, `g_glyphWidths` is 224 bytes, and `L2.eng` dropped every string with a byte above `0x7F`.** **[V]**

**The raise, from the instruction bytes.** After `add [g_drawY], eax` (`y += record[0x0D]`,
`0x00402A91`), `Glyph_Draw` (`0x00402A14`) does `cmp dword [ebp+8], 0x005AF8F0` —
`g_fontBody` — and `jne 0x00402B0E` past everything below. Then three times:
`xor eax, eax ; mov al, [ebp+0xC]`, `cmp eax, lo ; jl`, `cmp eax, hi ; jg`,
`dec dword [g_drawY]`, with `(lo, hi)` = `(0x61, 0x6D)`, `(0x73, 0x77)`, `(0x80, 0x84)` and the
decrements at `0x00402AC0`, `0x00402AE2`, `0x00402B08`. The argument is the index `c - 0x20`
that `Ui_DrawText` passes, so the raised **characters** are `0x81…0x8D`, `0x93…0x97` and
`0xA0…0xA4` — 23 — in **`Fntl2_14.pl8` only**. `Glyph_Draw` has nine call sites, all in
`Ui_DrawText`, all passing the same font and index, so both shadow passes and the drop shadow
move with the glyph. The test is the code and not the picture: `0x86` is `'a'`'s frame and
raised, `0x87` is `0x80`'s frame and raised while `0x80` is not.

**The table was missing 96 bytes.** `Ui_DrawText` looks up every byte above `0x1F`, so
`g_glyphWidths` runs to index `0xDF`: 224 bytes, ending at `0x004D72CF`, where another table
begins; no absolute address in the file points into `0x004D71F1…0x004D72CF`. Eleven of the 96
are glyphs — `0xA0…0xA7` and `0xDF…0xE1` — and our 128-byte `GLYPH_MAP` drew them as blanks;
five of the raised characters are among them. `pl8.md`'s *"highest frame the table asks for"*
was 104 for that reason and is 105, which every face still holds.

**`Eng::get` returned `None` for any string with a byte above `0x7F`**, under a comment saying
the file is Latin-1 and that `from_utf8` would reject those bytes. It ran `from_utf8`. The
English file has nine such strings — 295/2…295/10, the bullets of *"What should I do each
turn?"*, each opening with `0xB7` — and `group(295)` read two strings of eleven. `Eng` now
returns each byte as the `char` of the same number, which is the number `Font` indexes with.

**What could not be done as asked.** The brief wanted the raise measured on a real `L2.eng`
string that contains a raised character. **There is none**: the English file's only bytes
above `0x7F` are those nine `0xB7`s, which have no glyph, and the DOS install's `L2.ENG` has
no raised byte either. The raise is reachable only through a translated `L2.eng`, and none is
on this machine. The test measures it on the two shared-frame pairs, and asserts the file's
high bytes so that a translated file turns it red.

Tests: `shell::glyph_draw_raises_three_index_ranges_and_only_for_g_font_body` (compares and
decrements read out of `Lords2.exe`; every character `0x20…0xFF` put to
`font::accent_raised`), `shell::an_accent_sits_one_row_above_its_own_frame_in_the_body_face_and_nowhere_else`
(ink of `0x86`/`0x87` one row above `'a'`/`0x80` in body, identical in heading, small and
eight), `shell::the_each_turn_help_page_keeps_its_nine_bullets`, and
`shell::the_glyph_map_is_the_table_in_the_users_own_executable`, now over 224 bytes.

---

**C176 — every play path now behaves as one of the original's two `Sound_PlayFile`
shapes, and the verb that never dropped a request is gone.**

`Sound_PlayFile` (`0x00427990`) returns at once while the one-shot buffer, `DAT_00522AEC`, is
still playing. Ours did that only for the troop cries and `Wall_Smash`. `Audio::play_speech` never
dropped a request, and the fanfares and `fire.wav` went through `Audio::play_effect`, which neither
dropped a request nor took the buffer.

**The statement in front of each reproduced call site splits them into two shapes.** `[V]`

* **Plain `Sound_PlayFile`: drops while the buffer plays.** Now [`Audio::play_file`]. This covers
  the speech lines (`Panel_OpenRation` ×2, `Sidebar_Button`, `Panel_SplitButton`, `Map_ZoomOut`),
  the tips' chained takes (`FUN_004B3ACD`), `Msg_DrawWindow`'s five message fanfares and
  `Battle_ChooseSettlement`'s `ff_batl.wav`, `Panel_JobDetail`'s `fire.wav`, the six troop-cry
  sites via `Sound_PlayTroopCry`, and `Wall_Smash`.
* **`Sound_StopOneShot()` then `Sound_PlayFile`: cuts off whatever is playing, and is never
  dropped.** Now [`Audio::stop_and_play_file`], with [`Audio::stop_one_shot`] for the stop. This
  covers `Msg_PlayVoice` in all three bands, and setup page 4's line (`FUN_00432CC8` ×2 and
  `Setup_ChooseCampaign`, each `S011_02.wav`).

**`play_speech` is removed, so no call site can regain the old non-dropping behaviour.** A request
that is dropped no longer becomes the buffer's occupant, so the clip that was there stays the one
`Sound_OneShotBusy` asks about.

**A behaviour change a player can hear.** The message fanfares pass `isSpeech = 1`, so the
**Speech** switch now silences `ff_msg.wav` and `ff_capt.wav`, and the Sound Effects switch no
longer does.

**The battlefield's H and V order now works while paused.** `Battle_FormationKey` (`0x0043C77A`)
checks `DAT_00553C6C == 0 && g_appPhase == 3`, and its `WM_CHAR` caller checks
`g_battlePhase == 2 && DAT_0057A0CC == 0`. Neither is the pause word `DAT_0053F238`, which
`Battle_OrderClicked` tests for click orders. So H and V change the formation while paused, as they
do in the original.

**Tests**, each ablated and observed red on its own assertion:
* in `audio_screens`, a zoom-out line and `ff_batl` requested over the ration line are dropped and
  do not take the buffer, and requested after the line ends both play;
* in `audio_wiring`, a letter's fanfare holds the buffer, the lord's voice cuts it off, and
  Speech: Off silences the fanfare;
* in `tips`, a troop cry holds a tip's next take back until 64 ticks after the cry's last sounding
  tick, as the narrator does;
* in `audio_battle`, V and H change unit `+0x09` while paused.

`a_screen_speaks_once` was rewritten. Its mid-line comparison had been measuring the rewind a second
trigger caused, and the drop hides that, so it now plays the line out and listens again.

**At merge.** The film merge (C171) had added one more caller of `play_speech`, which this branch
never saw: the ending film's narrator line, `Msg_DrawWindow#21`. Its own comment names it
`Msg_PlayVoice(DAT_004F0374, DAT_004F0354)`, so it is `stop_and_play_file(name, true)` by the same
rule as every other `Msg_PlayVoice` site. C166's sentence about the one buffer being "set by
`play_speech` and `play_file`" now says that `stop_and_play_file` took `play_speech`'s place, here.

---

**C177 — The job popup's five bodies drew nothing, the event letters drew no count, and a county with no castle has barracks for 2500.**

C164 carried the figures and recorded three painters missing. They are built,
each from its decompiled body, with every word out of the player's `L2.eng` and
our transcription only as the fallback. **[V]** on every address, coordinate,
lead and suffix below; each suffix pointer was read out of the shipped exe.

| body | painter | calls | groups |
|---|---|---:|---|
| grain | `Panel_JobGrain` `0x00413590` | 27 | 77, 22, 8 |
| cattle | `Panel_JobCattle` `0x00413B30` | 29 | 77, 8 |
| reclamation | `Panel_JobReclamation` `0x004140F3` | 6 | 77, 8 |
| castle | `Castle_DrawStatusBlock` `0x0041DEDB` via `FUN_00414220` | 13 | 71, 8 |
| iron, stone, wood | `Panel_JobIndustry` `0x00412E6B` | 12 | 76, 8 |
| event letters | `Msg_DrawWindow` category `0x0F`, `00470000.c:2067` | 3 + 8 | the event's own, 77, 8 |

Every call site is reproduced except the event arm's two `Ui_DrawNumber(+0x2F8)`
lines and the words chained after them (Plague, Wedding fever): `+0x2F8` stays
excluded for C164's reason, and a word placed from a number that is not drawn
cannot be placed. The blacksmith's full page (`Panel_JobBlacksmith`, 21 calls)
and the job's `iconvill.pl8` picture remain unbuilt, as before.

**Two things the reading turned up that are not the painters':**

* **The event letter is unreachable in our engine.** `FUN_00448D7E` posts it —
  `Msg_Enqueue(0, g_localPlayer, county.eventId, 0, 0x0F, county, 0, 0)`, once a
  frame from the loop at `0x004B99C0`, for `g_selectedCounty` when its
  `eventFired` is set and it is the local player's — and no function of ours
  does; no `Message::Event` reaches the ring. The painter is tested by posting
  the record directly. `frame_of`'s comment also states the arm's height rule
  (`eventId < 0x12E ? 0xC0 : 0xE0`, the county's id, not the record's) and the
  code ignores it; the eight events here are all below `0x12E`, so nothing drawn
  moves, and it is recorded rather than fixed.
* **`Castle_DrawStatusBlock` indexes two tables one word low.** `&DAT_004D8A0C +
  type*4` and `&DAT_004D8A24 + type*4` are right for types 1…5; at type 0 they
  read `CASTLE_WORKFORCE[4].1` and `g_castleGarrisonCap[5]` — bytes `c4 09 00 00`
  and `00 00 00 00`. So the castle builders' popup of a county with no castle
  says *"Boosts tax revenues by 0 %"* and *"Barracks for 2500 troops."*, and
  `siege-aftersie.sav` holds such a county of the player's. `[V]` on the bytes
  and the painter, `[I]` that a player sees it; reproduced, and not yet in
  `docs/bugs.md`.

**Measured and not chased:** an ordered castle with its materials delivered and
an industry split of 100 is staffed by nobody, because castle building's share
at `+0x130 + 3*4` is 0 after `order_castle` and wood cutting takes the county.
Whether `Castle_Order` leaves that share alone too was not read.

**Tests** — `crates/l2-game/tests/job_bodies.rs`, ten, each figure and word in its
own box at the painter's coordinates. Stored non-zero: the grain store, eating
and overall change; herd, births, deaths, slaughter, the overall change and all
four crowding bands; industry output, efficiency and both blacksmith figures;
castle types 1–5's bonus and barracks, and type 0's 2500. Made non-zero by the
rule's own road: the sowing, the yield, `+0x2FC`, `crop[0]`, `crop[2]` (fields
painted, seasons advanced), both materials owed and the hundred-season estimate
(`order_castle`), `+0x278` and `+0x274` (`event::fire` and the season ticks),
`+0x24C` and `+0x270` (a hand-set weather band and the tick). **Only ever zero:**
field reclamation (`+0x204`, `+0x214`), the castle builders and so any finite
estimate, stone's output, and the fertility band's input.

Sixteen ablations went red at the box they name. **Two stayed green and are
findings:** the iron popup reading the wood record passes, because every save
here produces equal wood and iron at the same 80%; and reclamation's `'@'` lead
as `' '` changes no pixel, both being glyph-less.

---

**C178 — A window over a page is in the page's colours, and we asked the
window.**

A player, twice: behind the first tip on the raise-army screen and on castle building, *"the
screen behind it is color reversed. Immediately fixes after dismissing the tutorial screen and
doesn't return"*; and on a battlefield, *"it's all reverse color ...or..something. It's blue
grainy madness."*

**The original has one display palette and only a painter writes it.** `Screen_Armoury` ends
with `Palette_Set(armoury.256)`; `Battle_LoadAssets` sets `T32_bat1.256`. Nothing drawn over a
page touches it: `Tip_Show` (`0x00476DA9`) saves `g_screenId`, writes `0x27` and posts a
message; `FUN_00476E21` puts the byte back; `Msg_DrawWindow` (`0x0047309E`) calls no
`Palette_Set` in its 10,915 bytes. There is no dim, shade or remap table on this path at all.
`[V]`

`Machine::palette_name` asked the **top** screen, and the tip host and the message scroll —
like the menu bar, the options pages and save/load — name no palette. So any overlay turned the
page beneath it to the campaign palette. The tip made it look like a first-open defect only
because a tip shows once a game. It now takes the nearest screen that names a palette, looking
down through overlays and stopping at the first page, which is where `Machine::draw` stops.

**The battle report is a different defect, found while testing this one and not fixed here.**
`BattlefieldScreen::palette` names `T32_bat1.256`, and that file is read into
`Assets::battle` (`l2_view::scene::BattleAssets::palette`) and never into the shell's palette
map, which is the only place the presenter looks. The name resolves to nothing and
`.unwrap_or(&assets.palette)` quietly presents **every battlefield frame** through the campaign
palette, window or no window — since `75f08b8`, which is in the player's build. Nothing on the
battlefield path remaps indices, so the fallback is not harmless. This correction's own fix is
only that a window over the battlefield now *asks* for the battlefield's palette, asserted by
name in `tests/overlay_palette.rs`. A silent fallback on a lookup by name is what hid it: the
next palette a screen names and nobody registers will be hidden the same way.

**The test could not have been a canvas test**, which is the reusable part: every index on the
canvas was right, and the defect lived entirely between the canvas and the glass. The presenter
was in `main.rs`, where nothing can call it; it is `Machine::present` now, and
`crates/l2-game/tests/overlay_palette.rs` asserts presented colour at fixed pixels.

**A lead, not taken:** `main.rs` draws on the tick and presents on `RedrawRequested`, and the
palette is read at present time. An event that changes the stack between the two would present
the old canvas through the new stack's palette for one frame. `[I]` — winit's ordering was not
driven.

---

**C179 — `Tick_Pulses` resets its stamp, and the armoury carried the remainder.**

Three reports about the raise-army and armoury screens, read against the binary.

**The walker ran 1.6 times the original's speed.** `Tick_Pulses` (`0x004BBC80`) fires when
`0x13 < timeGetTime() - stamp` and then sets `stamp = now`, not `stamp += 20`. On a 16 ms tick
the pulse is every second tick, 32 ms. `Anim::tick` subtracted twenty and kept the rest — the
one reading under which the walk is exactly 200 pixels a second on every machine, which the
original reaches only on a frame of exactly 20 ms. That is the same reading `crate::press` makes
of `FUN_004B20ED`, the 30 ms gate beside it. `[V]` for the gate; *our tick is the frame* is
`[I]`, as it is in `press.rs`. **Every other `Tick_Pulses` consumer in the tree was written the
other way** — `industry.rs`'s `EVERY is PULSE_MS in 16 ms ticks`, the village — and is not
changed here.

**The walk is started by picking a rack, never by the `+`.** `FUN_004AABD8` has one call site,
`Armoury_ClickRack`'s first statement, with the type being *left*. `Levy_Seed` (`0x004AA90A`)
zeroes that type and the walk flag on every door into the armoury, so the first pick after a
door never walks: *"he did… after I picked something else?"* is the original. Two divergences
were ours: re-picking the open rack was refused (the `0x0D` arm tests no selected type), and a
re-seed left a walk on the floor. No tip can swallow the pick — `Tip_Update` has no `0x0A` or
`0x0D` arm.

**The levy slider was a click.** `Levy_SliderClick` (`0x00435CEF`) reads the arrows on
`pressed || doubleClick` and the track on the **level** `g_mouseLeftDown`, every frame; and
`WM_LBUTTONDBLCLK` sets no down bit. It had no `docs/arms.json` record, which is why the
inventory said nothing was missing.

---

**C180 — Five player reports were one unported function, and the lead that would have explained them all was measured and was not it.**

The reports, from builds `a5b112c` and `73df349`: *"industry values don't seem to update, and
for some reason mining started as off"*; *"the labor slider seems to reset each turn so that I
have to reassign peasants to wheat each turn"*; *"wheat does not show the +value when it is about
to be harvested, I'm noticing generally the industry numbers in the sidebar are inaccurate."*

**The lead first, because it was plausible and it was refuted by measurement.** C168 put
`Ai_ManageFarmsAll` at the head of the season, and an AI pass re-planning the person's county
would have produced every one of these at once. The original's gate (`0x0049A990`) is
`strength != 0 && isHuman == 0`, and ours tests the same two things. On England turn one and on a
new England, realm 1 has `is_human` set, and running the pass leaves the person's county's
labour, shares, industry share and fields byte-identical. `[V]`, measured on both paths.

**What it was: `Labour_Move` (`0x00439B52`) was ported as its first two lines.** The village's
drag and its double click (`Village_BalanceJob` → `Labour_Move`) moved the workers and stopped.
The function goes on:

```c
FUN_00439CC2(county, from, to);             /* men on a site switch it on */
Ration_Apply; County_RefreshEstimates; FUN_00448648(owner);
Labour_RecomputeIndustryShare; Labour_RecomputeShares;
Ration_Apply; County_RefreshEstimates; FUN_00448648(owner);
```

Each missing line is one of the reports, measured on England turn one's county 8 before the fix:

* **No refresh:** moving 36 foresters out left the wood row at `+86`, where the original shows
  `+57`. *"Industry values don't update."*
* **No `FUN_00439CC2`:** 36 men dropped on the mine left it switched off, its ceiling 0, its site
  idle, no iron row on the sidebar, and the season sent them home. C121 read the function a
  month ago and nothing built it. *"Mining started as off"* — see below for why it starts off.
* **No `Labour_RecomputeShares`:** the season's `Labour_Allocate` deals a county out from its
  eight shares and never writes one, so it dealt the old split back every season. *"I have to
  reassign peasants to wheat each turn."*
* **The missing harvest `+`** is the same shape: the grain tail (`0x0044D374`) matches ours arm
  for arm, and it was forecasting from a staffing the season had already undone. With the fix, a
  sown county forecasts `+432` on the turn before harvest.

`Kingdom::move_labour` is the whole function now, and `Industry_ToggleFromMap` gains the
`Ration_Apply` and the `FUN_00448648` it lacked. That last is the Readme's *"turning a blacksmith
on will reduce the resources available to other blacksmiths"*: on `siege-lastturn.sav` the second
smithy's ceiling halves from 180 to 90 on the click, where ours kept 180 until the season.

**Mining starts off, and that is the original.** `Game_SetupRealmsAndCounties` (`0x0049BD99`)
switches on one industry per start county: `for (i = 0; i < 4; i++) if (i != 2 && hasResource[i])
{ enabled[i] = 1; break; }`. Wood is record 0, so a county with a forest never starts with its
mine. `england-turn1.sav` agrees: five switches on, all forests, all in owned counties. Both our
paths agree with it — the importer (C57, not regressed) and `Scenario::from_map`. What was ours
was that the one road the original gives a player to turn that mine on without the map, putting
men on it, did nothing.

**And a new game had a second defect behind the first.** A new England opened with **no foresters,
155 idle and all four forecasts zero** in every start county. Nothing had computed an industry
ceiling before the opening season's `Labour_AllocateAll`, for two reasons, both `[V]` by call order:

* `Industry_ProduceAll` (`0x0044E852`) calls `Industry_LabourEstimate` after every production
  pass, and ours did not. A loaded game hid this, because the importer carries the last season's
  ceilings.
* `Game_SetupRealmsAndCounties` runs `Labour_Allocate; Ration_Apply; County_RefreshEstimates` twice
  per start county *before* switching the forest on, and ours did not run them at all. Every AI
  county is allocated again at the season's head by `Ai_ManageFarmsAll`; the person's is not. So
  his herd went through the opening season unminded: 47 head and −10 forecast. The save shows why
  the switch has to be off during those rounds: `Industry_ProduceAll`'s `symbols.json` note gives
  the realms' opening wood as *(0, 66, 66, 132, 166)*, and the 0 is the person's.

With both, **a new England's four AI start counties match `england-turn1.sav` exactly** for the
same realm, and the person's county matches the save's person's county job for job, ceiling for
ceiling and forecast for forecast, on a different seat. The four tests in
`crates/l2-game/tests/labour_move.rs` drive all of this through the village, End Turn, the setup
page and the map. Six ablations were run and each was red at its own assertion.

**One order is not the original's, `[D]`:** our industry passes run iron and stone over every
county before wood runs over any, where the original goes county by county. So the weapons
estimate made in the wood pass sees the whole realm's iron. It moves a smithy's ceiling for that
season's two allocations only, and `Panels_RefreshAll` recomputes it before anything is drawn.
Folding the passes would move `l2_game::save`'s pass indices.

**Open, and the reason each is open:**

* **"Generally inaccurate" may have a second cause we cannot measure.** `Industry_LabourEstimate`
  writes `efficiency = ramp(workers)` on every call, and C136 left that unported. With *Advanced
  Farming* off the ramp is a flat 80 and the write changes nothing. **All eleven saves on this
  machine have it off.** If the player plays with it on, the original's efficiency climbs several
  times a season (two refreshes per drag, three per switch, two more per season) and ours climbs
  once, so ours would under-forecast and under-produce. What would settle it: **two autosaves one
  End Turn apart from a game with Advanced Farming on**, and whether his game has it on.
* **The person's herd on a new England is 109 against the save's 101**, on county 11 against
  county 8. Every other figure of that county agrees. Not explained.
* **`docs/bugs.md` B12 looks wrong, `[D]`, not acted on.** It reads `Industry_ToggleFromMap` as
  toggling the castle share with the stale switch value. The decompilation's castle arm does that
  (`0x0043D309`, the call inside the arm) and then **toggles it again** with the new value
  (the shared call after the first refresh), so the share should end following the switch. A quirk
  default rests on B12, and this correction does not change it.
* **`Industry_ProduceAll` repaints every county's site tile, with no owner test, `[D]`.** Ours
  skips unowned counties. So a county that secedes keeps a working mine or forest on the map after
  `County_MakeIndependent` has switched it off: the picture and the switch disagree.

---

**C181 — a siege had no fire, no oil and no docked tower because five routines had
never been read, and the three documents that described them were wrong in ways that would
have built them wrong.**

C166 filed nine battlefield sound sites as blocked on mechanics. Six of them were five routines
of a siege, and this entry is what reading them end to end found. `docs/battle.md` §17 is the
mechanism; this is what was believed before, and what was built.

**Three descriptions were half-right, and each half would have been built.**
* `docs/audio.json` said `FUN_0047A814` is `Melee_Tick`'s oil arm. **It has a second caller**,
  `BattleUnit_Order`'s oil loop: a pot ordered downhill pours where it stands. And the first
  caller is reached because `Melee_AdjacentEnemyDir` (`0x004972F9`) **does not skip siege
  engines** — a man steps at a pot and the 999 arm puts it in melee. Built from the note alone,
  a pot would pour only if something engaged it, and nothing in `l2-sim` could.
* `docs/audio.json` said the bridge fire *"sets that cell and its neighbours burning"*. **It walks
  five rings along the bridge**, lighting only bridge cells beside something already burning,
  each seven frames longer-lived than the last. And its second caller is `Cell_TryEnter`: a
  besieger stepping onto a bridge sets it alight.
* `docs/battle.md` §6.3 said surfaces 10 and 17 are written by `0x00485675` and `0x00485861`.
  **`0x00485861` writes 0x10, which burns nobody**; `FUN_004859E5` turns it into 0x11 a frame
  later and floods the wood one ring a frame. And §14.3b filed the Readme's *"towers can not be
  moved again"* as unlocated: it is `FUN_00491492`, **which destroys the tower** and writes a ramp.

**Built**, in `crates/l2-sim`, as lockstep state: `fire.rs` (a fire is a class-5 record in the
same hundred slots as the arrows, remembering the surface it burnt; the oil stream is class 7;
the bridge fire; the wood fire and the AI garrison's fire arrows; `BattleMan_BurnTick`), the
tower half of `siege.rs` (the leading-edge test from `g_engineEdgeOrtho`/`Diag`, the dock search
from the polar facing, the ramp), the elevation-4 arm of the catapult strike, and their places in
`runner.rs`. Every new field is in `tests/lockstep.rs`' encoder, and the field census walks the
two structs that gained one. Six sites move from `blocked` to `reproduced`: **75 → 81 of 143**,
and reachable files **674 → 678 of 771**.

**Determinism**, proven over a siege that uses all of it: `l2_sim::proving` is a constructed
field — ours, like `our_castle` — on which a pot pours, a tower docks, a knight climbs its ramp, a
bridge and the men on it burn, and catapult shots bounce off a wall four high, inside two thousand
frames. A copy whose cue record is wiped every frame equals the untouched one at every frame; a
game with a sound `Director` listening equals one without at every frame and saves byte-identical,
with silent audio and with the install's.

**Our own castle cannot show two of the five**, and that is a finding about our layout, not about
the rules: a tower docks only against ground exactly two high and an AI's oil will not pour from
below two, and every wall `our_castle` draws is one high. `proving.rs` exists because of it.

**Found and not built**, each named in §17.8: men attack siege engines in the original (the 999
arm does not ask what it engaged) and `melee::engage` refuses them; rams and catapults still step
as one cell; the player's fire arrow; the engine side-step. One defect of the original is
reproduced and filed: `FUN_00485675` does not test its slot, so a cell set alight with the array
full burns for good — `docs/bugs.md` `B102`.

**The lesson is C140's, and the brief's.** A blocked row's note is a description written to
explain why something could not be built, and it is read for that and nothing else. Two of these
three were one clause short, and the missing clause was the caller.

---

**C182 — The press timer is one per widget record, the marker is now the kind
itself, a double click never holds the button down, and four yes/no boxes answered raw
clicks.**

Three gaps C148 and C165 left in `crates/l2-game/src/press.rs`, and two more a player
reported while they were being closed. All of it is `[V]` from `Widget_Test` (`0x0040DA1E`),
`App_WndProc` (`0x004B29BE`), the per-frame latch `FUN_004B191E`, `Screen_FrameInput`'s arms,
`Screen_DrawWidgets` and the exe's own kind bytes (`node tools/oracle/kinds.js`).

**One pending press was one too few.** `Press` held a single timer and a single pending
widget. `Widget_Test`'s countdown loop walks **every** record, decrements each one's `+0x0D`,
and calls each kind-5 handler that reaches zero without returning — so two gauntlets pressed
a few frames apart both act, each on its own twentieth frame, and the same record pressed
twice restarts its own count and acts once. Ours dropped the first of two, drew only one of
them down, and a spinner pressed while a thumb waited **cancelled the thumb**. `Press` now
keeps a timer per record; `tick()` returns every handler owed, countdown first; `pressed()`
is gone and `is_pressed(i)` is per record, so the compiler found every painter.

**`arms.rs` could not see the declared kind, and now there is nothing separate to see.** C165
measured it: every options row declared `Kind::Press` under a `left-press-delayed` comment,
and `arms.rs` green. A widget's kind is now written `crate::arm!("<id>", Delayed)` — which
expands to `Kind::Delayed` and nothing else — and `arms.rs` reads that token as the marker,
taking the gesture from the identifier through `Kind::gesture`. A comment marker may not claim
`left-press-repeat`, `left-press-delayed` or `left-press-held`, since only a `Kind` handed to
`Press` answers those. Both ablations run: the row declared `Press` in its `arm!` turns the set
check red; a bare `Kind::Press` under the old comment turns the new rule red, naming the line.
**What remains possible**: a record filed `left-press` over a widget declared bare
`Kind::Repeat` with its own comment marker; the exe-gated kind-byte check is what catches that
one.

**A double click does not hold the button down.** `App_WndProc` answers `0x203` with
`DAT_004EADA1 |= 1` and nothing else, so `g_mouseLeftDown` stays clear however long the second
press is held, and its `WM_LBUTTONUP` raises no release. A kind-4 widget fires once and never
repeats; ours started the auto-repeat. And `docs/input.md` counted six screens that dropped
`Event::DoubleClick`: five did — diplomacy and its compose dialog, divide, the information
panel, the message scroll, supplies — and the battle prompt never had. Each now answers where
the original's guards do and nowhere else; `docs/input.md` §5 has the table. The county panel's
C156 double click still sets `slider_held`, which by the same fact the original would not —
**not changed here, recorded**.

**A held arrow did not repaint.** *"The click and hold seems to increase the value but it is
not visually shown until release."* The original paints on the frame the handler runs —
`Tax_IncreaseCounty` ends `Panel_Tax()`; the gift stepper and `SaveLoad_Scroll` set
`g_redrawRequest = 2`; `Battle_Frame` runs `Screen_DrawWidgets` every frame for the divide
rows, the supplies numbers and the trade panel. Ours repaints when dirty and a repeat is not
an event. `Press::take_redraw`, handed up by every screen that owns a `Press`.

**Four yes/no boxes answered raw clicks** — found by enumerating every exe record drawn with
frames 29 and 31, after a player met two of them (*"no sound on clicking the yes/no on save,
and it is still on mousedown instead of mouseup"*):

| box | kind | what ours did |
|---|---|---|
| save/load, `g_saveLoadWidgets` | 4 | saved on the click, no click, no picture — and **no 150-frame wait**: `FUN_004342F3` only sets a latch, `SaveLoad_Tick` writes `0x96` frames later |
| castle build, `g_castleBuildWidgets` | 5 | acted on the click |
| raise army, `DAT_004DD340` | 5 | Continue, tick and cross acted on the click |
| trade panel, `DAT_004DD838` | 4 | no click, no picture, no repeat — and the arrows step **ten** from repeat step `0x2C` |

The player's *"mousedown instead of mouseup"* on the save box is the one report here that the
gesture does not answer: kind 4 **is** the press. What he remembered is the wait.

**And one crash nobody had reached.** `supplies.rs`' `THUMB_UP_INDEX` was the literal 6, from
three rows of spinners, over a `ROWS` of two: either thumb fell into the spinner arm and
indexed a third row, twenty ticks after the press. It is `ROWS.len() * 2` now. Nothing had
fired a thumb through the screen until a double-click test did.

Fourteen `docs/arms.json` records. The ablations are in four groups and every one went red
where its test says; the list is in each test's own doc comment.

**The slider and the widget table share one `handle`, in the original's order.** C179 gave the
recruitment slider its own `DoubleClick` and `Release` arms, and when this branch's table arrived
they matched in front of it and the compiler called the widgets unreachable. The order is not a
matter of taste: `Screen_FrameInput` (`0x0042FF10`) is
`if ((Msg_HandleInput() == 0) && (Screen_HandleInput() == 0)) { …the fifty arms… }`, and
`Screen_HandleInput` (`0x004BA9C8`) is the per-screen **widget** pass while `Levy_SliderClick`
(`0x00435CEF`) lives in the `0x17` **arm**. So `DAT_004DD340`'s three kind-5 records are asked
first and the slider gets only what they did not take — not the other way round.
`RaiseArmyScreen::widget_press` is that hit test and its `bool` is the original's
`Screen_HandleInput() == 0`. Three consequences fall out of the same reading and each is now
written where it happens: `left_down` is set by **every** press, because `WM_LBUTTONDOWN` sets
`g_mouseLeftDown` whatever the press landed on, so sliding off Continue onto the track still moves
the knob; the right release meets the slider with no widget pass in front of it, because
`Screen_HandleInput` is left-button only; and a release reaches both halves, ending the record's
hold and clearing the level in one event. The two never collide geometrically — the band is
x `0x80…0x176` by y `base+0x10…base+0x40` and the records sit at (480, 336), (352, 256) and
(400, 260) — which is why nothing caught the order being wrong.

**Found and not fixed: the castle chooser is stranded by its own film, and it predates this
branch.** `CastleBuild_Confirm` hands `Smk_Play` a return screen of `0`, so in the original the
end of the film *is* the map. Ours pushes the film over the chooser and leaves the chooser to pop
itself on its next `update` — and `Machine::update` runs `run_tips` and `pump_messages` **before**
that update. `tip::DELAY` is `0x14` frames, so on any film longer than twenty frames the castle
screen's own advisor tip (`Tip_Update`'s `0x1B` arm) is seated the instant the film goes, the
chooser never gets its `update`, and the player is left on the chooser under a tip. **Measured
without this branch's timing anywhere**: pushing `Film::Castle(0)` over the chooser directly, as
`main`'s `confirm` did, and letting the real `castle1.smk` play strands it at tick 523 of 521
frames of film. `movies.rs` asserted the clean end state only because placeholder assets fail to
open a film and the whole sequence fitted inside twenty frames; the two tests now assert the film
is off the stack and say in full why they stop there. The fix is `g_smkReturnScreen` as a
transition — *"go to screen X"*, unwinding the stack — which is the machine's vocabulary and not
the input model's, so it is reported rather than smuggled in here.

---

**C183 — the battlefield was "blue grainy", ghosted and snapping, all three had
separate causes, and the test that held the merge up for a day was the *films*, not the siege.**

A player reported three things on one battlefield: the colours were *"blue grainy madness"*, men
*"reset on their square once as they move"*, and they left *"ghosting"*.

**Colours.** `Res_LoadStatic` (`0x00499859`) preloads `t32_bat1.256` and `t32_stn1.256`, and
`Screen_DrawBattlefield` (`0x004233F7`) ends with `Palette_Set` (`0x004B0AB5`) for a field battle
or a siege. That is a plain copy with no remap or shade table. Ours named `T32_bat1.256`, but
`shell::PALETTES` never registered it, so the presenter silently fell back to `base01.256`: 209,925
of 215,040 field pixels were wrong. Grass index 77 came out `(0, 97, 190)` instead of
`(64, 85, 12)`. It is registered now, and a message or tip over the battlefield resolves to it
through the same overlay rule as every other page. **Not the same cause as the tip backdrop.** The
siege arm (`t32_stn1.256`) is not ported, because its tileset is not loaded either.

**Ghosting.** The original clips men and horses to x 0–480, y 24–472 (`FUN_004BC020`).
`BattleFigure_Draw` and `FUN_004BE4DF` clip every sprite to that rectangle. Ours drew them unclipped
into the menu bar, the right column and the bottom strip, which nothing on the battlefield repaints.
`scene::FIELD_CLIP` is that rectangle.

**Snapping.** `BattleMan_Step` (`0x0048F1DD`) enters the next cell first (`FUN_00491B1F` moves the
map position), then counts `walking` 1, 3 … 15, and `BattleFigure_Draw` draws the man trailing
behind the cell he is already in. Ours drew the offset from the old cell, so each man was drawn up to
28 px behind his own square and then jumped a cell. A walking man is now drawn from the cell his
facing points into. Jumps of 16 px or more fell from 1,317 to 504. **The remaining 504 come from our
simulation stepping before entering and re-choosing direction every tick**, which the original never
does. That is left alone, because it changes battle outcomes, and it is documented in
`docs/battle.md` §13.6.

**Drawing does not change the battle.** `LiveBattle` is equal every tick, and `save::encode` is
equal: on `main` at `8a1b094` the painted and the unpainted play both fingerprint
`fnv1a dbe6c8fd57b4994a` (63,308 bytes). The honest limit of that second half, stated because the
first reading of it was too generous: `l2_game::save` is `l2_kingdom::save` plus a ten-field prefix
and holds **no `LiveBattle`**, so those bytes say the campaign around the battle came through
untouched, and the battle itself is carried by the per-tick `LiveBattle` equality and nothing else.
`tests/battle_picture.rs` drives a real battle through the screen stack and asserts pixels: the
palette's colours against the install's own `.256`, the man found by exact frame match advancing
every tick, nothing outside the field written, and the square he left equal to a fresh terrain pass.
Registration ablated is red.

**The red test was C171's films, not C181's siege work, and the suspicion is refuted by
measurement.** `painting_the_battlefield_with_its_artwork_does_not_change_the_battle` panicked with
*"a live battle"*: it read its casualty count off `g.battle` after 1,500 painted ticks and the
battle had been handed back. Built against `3774de3^1` — `main` the commit before C181 — that
battle ends at the same tick 655 with the same `Conclusion { winner: SIDE_B, cause: Annihilation }`
and is handed back at the same tick 902. C181 cannot be the cause: with no woodland, bridge, wall,
pot or engine on the field, every arm it added is unreachable here. The films landed *after* this
branch's base (`522bd8d`). `Battle_CheckOutcome` (`0x00477DFC`) ends a field battle the frame a
side's man count reaches zero, then counts `DAT_00568470` to 5000 behind the banner;
`Smk_OnFinished` sets that to 5001 when the outcome film ends. No film on the base meant the banner
kept its 5000 frames and 1,500 ticks still had a battle to read; on `main` the film opens and
`turn::finish_battle` takes it at 902. **The expectation was wrong and the simulation was right**:
the test keeps its claim — the two copies equal every tick — and counts casualties while there is
still a battle to count them from. The placeholder run, whose film cannot open, holds its battle
for all 3,000 ticks, which is the same rule seen from the other side.

**Measured and left alone: the ordered army always dies.** Seven seeds of that scenario — three
swordsmen and three archers ordered onto three crossbowmen and three macemen over eight cells of
open ground — annihilate the human side every time, 24 men to 0, the AI losing three or five with
only two or ten of those deaths in melee. Unchanged before C181 and on this branch's base, so it is
neither's; no claim is made here about what the original would do with the same twelve figures.
Written down because the determinism test is built on that battle.

**At merge.** The branch reverted its own presenter change so as not to collide with the overlay
palette's `Machine::present` (C178); what remains is the registration, which composes with that
rule. **C178's sentence that `Battle_LoadAssets` sets `T32_bat1.256` is wrong by the same reading:**
`Battle_LoadAssets` (`0x004987B7`) hands the tile renderer its geometry, and the palette is
`Screen_DrawBattlefield`'s `Palette_Set`. The two comments that repeated it, beside
`Machine::palette_name` and at the head of `tests/overlay_palette.rs`, are corrected here.

**Found and not built: the black overview panel is a missing painter, not a palette fault.** A
player on `main` without this branch also reported *"a black minimap"*. Registering the palette
cannot touch it — `ink.background` is the index nearest black, so our `fill_rect` is black in every
palette. The original paints the panel in `FUN_004BC51A` (`0x004BC51A`), registered by
`Battle_LoadAssets`' `FUN_004bc107(…, 0x1E0, 0x18, 2)` and scheduled four rows a frame by
`FUN_004BC1D1` out of `Battle_Frame`: a terrain raster of 2 × 2 tiles from `t2_bat1.pl8` (252
frames, one per `T32_bat1.pl8` tile), a 2 × 2 man from `t2_spri.pl8` over each occupied cell in the
**owning realm's shield colour**, and no viewport rectangle. Neither `t2_` sheet is loaded in this
tree; ours fills flat, colours by side, and draws a frame the original does not. `[V]`, from the
decompilation; left for a row of its own.

---

**C184 — C134 gave every unit a sub-tile counter and nothing drew it; a march ended a
tile early; and the gold ball of action had never been built.**

Two player reports against the same screen. On `A5B112C2B`: *"The army marching animation is
jumping from square to square, I remember there being an animation and some interpolation
between walking squares."* On `73DF34969`: *"The balls of the army movement are missing the
gold ball of action, it's just a grey ball like I can't get there when I attack a town."*

**The walk.** `Map_DrawArmies` (`0x00408438`) adds `table[zoom][facing][+0x149]` to a
unit's anchor, out of six 8 × 16 `i8` tables at `0x004D8108` … `0x004D8388` — carried now,
generated from the file and asserted against the user's copy. The unit's tile is already the
destination (`Unit_MoveInFacing` runs at the commit), so the offset drags the figure back;
the commit writes `+0x149 = 1` and admissions add 2, so a crossing is drawn at the eight odd
indices; and index 1 is 28 against a pitch of 30, so the figure hops two pixels at each
commit. **[V]** on the bytes, twice (the file, and our projection agreeing with every
heading); **[D]** on which indices are drawn. **The frame is one tick behind**: all four tick
handlers write `+0x07` before `Unit_Step`, which we reproduce outside the kingdom
(`l2_game::game::UnitFrames`) because a frame index is not the world's. `+0x1B` is not
stored: `Unit_StepOnce` is its only writer and moves it with `+0x149`, so it is always
`+0x149 / 2`.

**What ours said, and why nobody looked.** `campaign::draw_unit`'s doc said the tables had
nothing to index with — true until C134 — and that *"a unit mid-step would sit at its
destination tile in the original for the same reason it does here"*, which was never true.
The second clause is what made the gap look like fidelity. **A correct explanation of why
something is absent keeps being read after its premise has gone** — the same shape as
C123's comment above an unported tail.

**A march ended one tile early, in the simulation.** `movement::step` wrote `moving = false`
on the commit that emptied the path. `Unit_Step` (`0x00465D28`) does not: the commit returns
1, `moving` stays 2, the unit crosses the last tile, and the latched arm stops it at that
tile's edge. Drawn, ours left every finished army parked at `+0x149 = 1`, a whole tile back,
and re-walked that tile on its next order. **Fixed in the simulation**, which moves the
digest: a march now ends 8 road ticks (32 open) later. Three tests went red, and two were
asserting the defect's numbers — `the_wait_follows_the_units` (17, now 25) and
`a_unit_loaded_with_no_allowance_still_walks` (49, now 57). The third was this correction's own
presentation field reaching `Game`'s equality, now excluded from it. **[D]**

**The ball.** `Map_DrawPathMarker` (`0x004081A6`) tests `local_14 = flags & 0x50`, or `0x80`
with terrain other than `0x14`, *before* the cost, and draws `0x4E` — no owner, no reach, no
unit. Ours had only the cost arm, and a town's cost (100) is always past the budget. Its
placement was ours too, centred where the function adds `(0x14, 6)` and never centres. **An
enemy army is not an action tile**; the report's *"attack"* is narrower in the binary than in
the word.

**Still not done:** `Map_DrawArmies(mode)`'s two-pixel shift on the first cell of an offset
row; `SUBTILE_STEP_NET`; and our painter still draws every unit in one pass after every tile,
where the original draws each in its tile's lattice slot, so a figure walking back over a
later-drawn tile's overlay is on top in ours.

**The moved trajectory took a test with it, and that assertion was asserting an accident.**
`long_game.rs`'s `four_hundred_turns_of_england_reaches_the_rules_nothing_else_can` claimed
`Realm_SecedeIsolatedCounties` (`0x0044AE3C`) is reached in 600 turns of England. On the new
trajectory the human realm falls on turn 28, one realm ends with 13 of the 14 counties, nobody
is ever cut in two, and `SECESSION` never fires. The pass takes a county only from a realm
holding two or more contiguity blocks, and contiguity is the county neighbour list at `+0x5C`
and nothing else (`docs/kingdom.md` §6.1). Measured off the fixture's own lists — fourteen
counties, 39 undirected edges — **county 2 is the only cut vertex on the map**: county 1's list
holds nothing but county 2, and removing any other county leaves the remaining thirteen
connected. **[V]** So the pass can fire here only when a realm holds county 1 and something past
a county 2 that is not its own — enemy or neutral alike, `Territory_ExtendBlock` joining through
`County_IsNeighbour` (`0x00467E2C`) on same-owner adjacency. That is a fact about where the
armies went, not about a rule: a reachability assertion resting on one articulation point of one
map is a trajectory assertion wearing C27's clothes, and it had already survived two trajectory
changes by luck.

So the claim moved rather than being stretched or deleted.
`a_realm_cut_in_two_loses_the_far_half_through_the_turn_machine` deals the split in one line and
plays it through `l2_game::turn::end_turn` — the part `crates/l2-kingdom/tests/secession.rs`
never covered, that file driving the pass over synthetic chains only. What stays in the long run
is the invariant the pass maintains: after every one of 600 turns no realm is left holding two
blocks, measured by a new `max_blocks` census column, which is also what tells *"nobody was
split"* from *"the pass is not running"*. Not asserted, only measured: the mutiny fires again at
turn 100 and a tax rate at or above 20 at turn 78, which C134's table in that test calls
unreachable.

---

**C185 — the drag selection box is `Village_DrawBand`, and *"we could not find it drawing"* was spent as *"the original does not draw it"*.**

A player on `ee0cb92`: *"the drag selection box has disappeared, it was probably a debug thing
that you removed with other debug boxes."* It was not a debug thing. C173 put every marker,
outline and line of ours that the original does not draw behind `Prefs::debug_overlay`, and the
village's rubber band went with them on this sentence, which was in the source beside it:

> `Village_BandStart`'s hit region is read out of the binary and is wider than the picture — x 0
> … 0x1FF, y top … top + 0x178 — but **nothing in the decompiled corpus was found *drawing* the
> band**, so the outline is ours and so is its colour.

**`Village_DrawBand` (`0x00412795`) is the corpus drawing the band**, forty lines below
`Village_DrawCluster` in the same region. It was missed because it is not reached from
`Village_Draw`: it is a per-frame painter that opens on a **screen-id guard** rather than on a
name that says *village*. The body, whole:

```c
/* admits 0x02, 0x05 and 0x06, then rejects 0x02 and — after FUN_004120E0
   restores the saved 480 x 320 band — 0x06. So only 0x05, the band, draws. */
if      (x0 < 0)                          { w += x0; x0 = 0; }
else if (0x1FF < x0 + w)                  { w = 0x200 - x0; }
if      (y0 < g_villageTopY)              { h -= g_villageTopY - y0; y0 = g_villageTopY; }
else if (g_villageTopY + 0x178 <= y0 + h) { h = (g_villageTopY + 0x178) - y0; }
FUN_00403cf4(x0, y0, w, h, 0x20);
```

`FUN_00403CF4` is now `Ui_DrawRectOutline` — four `FUN_00403A8F` lines in one colour, twenty-two
call sites — and `0x20` is `rgb(255, 255, 255)` in `Base01.256`, the palette
`Screen_DrawCampaign` sets and the village never replaces, because `Village_Draw` paints over the
campaign screen rather than clearing it. So the band is **white** and ours was amber; and the
clamp is the original's, both branches written as `else if`, so a band that starts left of zero
is never clamped on the right. All of it is reproduced, with the install's index used where the
install's chrome is loaded and our own ink where it is not — the rule `county::draw_strip`
already follows for `CountyStrip_Draw`'s black `0x3F`.

**Its twin was already right, by luck.** `0x0041298A` — now `Battlefield_DrawBand` — is the same
routine for `g_screenId == 0x2A`, and `battlefield.rs` draws that band ungated because that
screen was written after C173. One band gated and its identical twin not is the shape of the
error: the gate was applied per *call site*, from a search for the painter by name, and the
search missed one of the two.

**What stays gated, checked one at a time.** The other twenty-odd sites C173 gated are ours: the
county-anchor and field squares on the map (`Sprite_TopIt`'s farm arm is the pasture herd and
nothing else, and the crop is the tile's own artwork, which `MapScreen::field_graphics` now
paints — so those are the *"a lot of the little orange/brown squares around icons"* the same
player also noticed going, and they are the *"debug squares still on the town square on map and
the fields"* an earlier report asked for); the garrison marker, whose banner `draw_flags` flies
from `FUN_004071A0`; the selection ring, against which the original's answer is the flood fill;
the besieger dot, whose original is `Flags1a.pl8` frame `0x82` over the *castle* and is still a
missing draw; the sidebar focus outline, which a player asked for by name; the End Turn hover
colour, which `Screen_DrawEndTurn` passes as `0x16` whatever the pointer does; the county strip's
quadrant outline, over quadrants the original leaves invisible; and eighteen status lines and
*not built* stubs, in our own font, on screens whose originals carry no such words.

**One hole this opened and did not close.** `Screen_DrawCampaign`'s far-zoom arm is
`Ui_DrawBox(0, 0x19C, 0x1E, 4)` and then four draws into it — `Eng_DrawString(0x65,
g_scenarioIndex, …)`, `Eng_DrawString(0x22, 0, …)`, `Ui_DrawYear(g_year, …)` and
`Eng_DrawString(0x22, 1, …)`, in `&g_fontHeading` at colour `0x3F`. Our status line sat in that
box and is gated, so at the far zoom the box is now **empty**. Neither picture is the original's:
the box wants the map's name, the season and the year out of `L2.eng` groups 101 and 34. Filed
rather than invented.

> **Closed by C189**, which also corrects this paragraph: there is **no season** in
> that box. The painter makes three heading draws — the map's name, *"Year"* and `Ui_DrawYear` —
> and a fourth in the body face, and none of them is a season. A hole filed from prose rather
> than from the listing carries the prose's errors into whoever closes it.

**The lesson.** *"Nothing was found drawing it"* is a statement about a search, and C173 spent it
as a statement about the binary. The two differ by exactly the strength of the search, and this
search was by name in a corpus where the painter carries a screen id and not a name. Rule 5's
*"we could not find it"* is a finding to report — and a finding is a thing to hold at arm's
length, not a licence to switch a draw off.

---

**C186 — a stand-in for an absent player was installed on the path a present player
uses, and the player watched his own army charge without him.**

A player on `ee0cb92`: *"my men in battle started moving before I clicked"*, then *"I actually
didn't see the enemy's units moving hence me thinking enemy ai on my troops"*, then *"it might be
that enemy ai is getting applied to my units"*. The first hypothesis — our battle AI runs on the
wrong side — is **false, and was measured rather than argued**: `tests/military.rs`
`the_battle_ai_thinks_for_the_enemys_units_and_not_the_players` passes unchanged on the base
commit. `Battle_UpdateAllUnits`' guard is reproduced exactly, the attacker raises as army A on
side 4 and the defender as army B on side 0 (`Battle_InitArmies`, `0x0047EFEE`), and no handler
ever ran for a human unit.

**What actually moved them was ours, in `l2-game`, and it was documented as a deliberate
deviation.** `engagement::begin_fight` — *"`Battle_Start` minus the screen"* — ended with

```rust
for (side, human) in [(SIDE_B, a_human), (SIDE_A, d_human)] {
    if human { let enemy = runner.home(other_side(side)); runner.order_side(side, enemy.0, enemy.1); }
}
```

under the comment *"This is the click a player makes on the first frame."* It was written for the
**headless** battle, where there is no player to click, and `begin_fight` is shared with the
**watched** one, where there is.

**`Battle_Start` (`0x004778A0`) orders nobody.** `[V]`: it runs `Battlefield_Build*`,
`Battle_InitArmies`, `FUN_00480F8B`, `Battle_UpdateAllMen` and `Battle_UpdateStrengthAdvantage`
and returns; there is no `Order_*` call in its 655 bytes. `Battle_RaiseSide` (`0x0047FEA7`) →
`BattleUnit_Create` (`0x00480662`) gives every figure a `tg x`/`tg y` equal to the cell it stands
on. And it writes `DAT_0053F238 = 0xFFFFFFFF`, so a battle opens **paused** — which is why this
was visible at all: the player saw the first frames and could tell the movement was not his.

**An unordered unit is not inert, which is the half worth naming.** `BattleMan_FireMissile`'s
`Missile_FindTarget` arm carries no human guard, so a player's archer stands on its cell and
shoots whatever comes inside its range. *Standing* is the behaviour; helplessness is not. The
player's own testimony — *"units don't advance on their own"* — is the binary's arithmetic as
well.

**Fixed by moving the stand-in one level out**, into `engagement::fight`, the headless path, as
`charge_for_the_absent_player`, with the reason on the function rather than in a comment on the
line. `begin_fight` is now `Battle_Start` and nothing else. A headless battle is byte-for-byte
what it was — `tests/seam.rs`' fought battle is still 3,100 ticks, 178 → 0 against 182 → 22 — so
no lockstep digest moved.

**The second report was true and was not a defect.** The enemy really did stand still.
400 peasants against 200 puts `Battle_UpdateStrengthAdvantage` (`0x0047FC01`) near −50 against
`g_aiAggressionThreshold` of 5, so every field handler takes its cautious branch; with no attacker
in `hit_memory` and nothing inside a charge radius of 8 or 9 cells, the only movement left is
`Order_ToRallyWaypoint` — and **a rally waypoint is the side's own deployment marker**. `[V]`:
`Battlefield_BuildRandom` fills all three of a side's waypoints, both rally groups, from the tile
it just found the marker on. A cautious AI orders itself to stand where it is. The player must
come to it. `tests/military.rs` `an_outnumbered_ai_holds_its_ground_and_does_not_advance`.

**Two measurements in the tree were this, and one attribution in a held branch was wrong.**
The ledger's `ordered-army-always-dies` — seven seeds, the human side annihilated 24 to 0 while
the AI lost 3–5 — was the ordered side walking into a standing, shooting defence; only the human
side was ever ordered, so *"the ordered army"* and *"the player's army"* were the same set and
nothing distinguished them. And `worktree-agent-a55ac6f31e368d8c9` was held on *"defending AI
archers walk to 13 cells instead of standing and shooting"*. **That branch measured no AI unit.**
Its only measurement of 13 cells is a unit test whose walking archer is `human: true` and moved by
`order_unit`; its field evidence is a flipped verdict in `seam.rs`, which records no side, state
or position. Its own `send_figure` fix routes `order_side` — that is, the very order `begin_fight`
was injecting into the player's army — into `State::Shooting`, and its `close_to_attack_tick` has
no side test on the walking arm. The archers that walked were as likely the player's. The branch
is untouched and still held; what changes is that its premise now has to be re-measured against a
`begin_fight` that orders nobody.

**The rule this is under is rule 5, and the failure mode is the one rule 5 exists for.** The
deviation was *declared* — the module header said `fight` issues *"the one order a player always
issues"* — and declaring it is what made it invisible. A stand-in for a missing input belongs on
the path that is missing the input, never on the one where the input arrives.

---

**C187 — Two cattle reports, one defect and one faithful quirk, and the
difference was only decidable because both saves were read.**

Two reports arrived together and looked like one bug:

> *"I don't know why 16 cows are being lost this season."*
> *"It shows idle and +8 cows, then I click the slider towards industry, then suddenly
> +12 cows, should be less since I moved toward industry instead of farming."*

**They are not one bug, and establishing that was most of the work.** Both saves were loaded
and diffed field by field. The first county holds 80 head on eight pastures with 114
milkmaids and 150 people; the second holds 99 head with 259 milkmaids, 48 idle and a cattle
ceiling of 303. Those are different diseases with the same presenting symptom — a cattle
forecast the player could not account for.

**The defect: `Herd_LabourEstimate` (`0x0044DD4D`) fills two words and we took one.** The
loop over `workers = 0 … population` writes the growth-maximising staffing to `+0xD8`, which
`Labour_Allocate` fills up to and which we had, *and* the **first staffing at which births
stop trailing deaths** to `+0xD4`, which we never wrote at all. `County::labour_wanted[1]`
held `County::new`'s zero in every county of every game this engine has ever played.

That word is the herd's only distress signal, and it has **four** readers, all interface:
`Panel_JobDetail`'s red worker count (`screens/info.rs`, `screens/job.rs`), the county
strip's *short* produce frame (`screens/county.rs`), `Village_RebuildIcons`' unselectable
shortfall icons (`screens/village.rs`) and the minimap's labour overlay band
(`County::minimap_bands`). All four were reading a zero, so all four said *nothing is wrong*.
His county wanted **149** milkmaids and had 114 — 47 % staffing, which
`land::herd_growth` turns into seventeen extra points of death rate, and there is no
arrangement of 150 people that would have tended 80 cows. The game had a way of telling him
that and we had disabled it.

**`[V]`, and not against one save.** Every original save stores `+0xD4` for every county, so
the search runs against the game's own answers with nothing inverted.
`crates/l2-kingdom/tests/cattle.rs` sweeps **every `.sav` this machine can open** — twelve
positions, both words, every county — and the fallback arm is live in three of them:
`lastturn.sav`'s county 1 stores 302 and 302 because its herd cannot break even at any
staffing its county could supply, and the least-bad count *is* the argmax, so floor and
ceiling coincide. Two other positions store 153/153 and 173/173. One arm was `[I]` and stays
so: no save holds a county with no people, so what the floor reads when the loop never runs
is unobserved; `LABOUR_NO_FLOOR` is used because that is what every other job writes for
*no requirement*.

**This moves the lockstep digest.** `labour_wanted` is inside `l2_kingdom::save`'s canonical
encoding (VERSION 5 put it there), so `counties[id].labour_wanted[1]` moves from 0 to the
break-even staffing on every county with `pop_band != 0`, at both of `Herd_LabourEstimate`'s
call sites. Nothing in the simulation reads it — `Labour_Allocate` reads `+0xCC + slot*0x0C`
and never `+0xC8 + slot*0x0C`, verified by exhaustion — so no rule changes; the digest moves
because the field is state and is hashed.

**And that is why one ablation had to be run a different way.** Deleting the write from
`Kingdom::herd_season_tick` and running a whole season is **green**: `Panels_RefreshAll`
rewrites the record afterwards and nothing downstream reads it in between, so the two call
sites are indistinguishable from the end of a season. The test that distinguishes them runs
`Pass::HerdSeasonTick` **alone**. Writing only half of a two-word record is the deviation
whether or not anything can see it, but *"the ablation is green"* is a fact about the test
and is written beside it rather than quietly not mentioned.

**The second report is the original's, and the branch left it alone.** The mechanism is
`[V]` off `Season_Advance`'s call list (`docs/kingdom.md` §3.4): `Population_UpdateAll`,
then `Labour_AllocateAll`, then `Panels_RefreshAll` — and `County_RefreshEstimates` lives
inside that last one. The cattle ceiling is bounded by the loop's `workers < population`, so
on a county that wants everybody on cattle it **rises with the population** — and it rises
one pass *after* the allocator has already dealt the newborns out. They land in Idle, and
the next `Labour_Allocate` to run for any reason hires them. Traced on his save: the ceiling
goes 305 → 344 across `RefreshEstimates` while the labour stays at 305, leaving 53 idle; the
slider click re-allocates and the forecast rises. Moving towards industry really does buy
him milkmaids. `docs/bugs.md` **B103**.

**One thing on that arm was ours.** `Kingdom::set_industry_share` named `FUN_00439122` — the
*drag* handler — and county `+0x2C`, and ran `Labour_Allocate; County_RefreshEstimates`
**twice** with no ration pass. `Labour_SetIndustryShare` (`0x0043933B`) writes `+0x08` and
runs `Labour_Allocate; Ration_Apply; County_RefreshEstimates`, once each. The omitted
`Ration_Apply` is the one that bites: `herd_eaten` sizes the herd the estimate that follows
searches over, and the forecast subtracts it twice. `Ration_Apply` is
`ration::preview` here for the reason `Kingdom::set_ration_wanted` gives — it records and
does not spend, and a slider dragged a hundred times in one gesture would otherwise eat the
county. Fixing it does not change his number and the correction says so.

**The finding under both of them.** The two spare words of the twelve-byte labour record were
imported as nothing at all, and C151 already recorded the *ceiling* half of that — three
sidebar forecasts that were the tail of an estimate pass ported without it. This is the same
omission one word to the left, found four hundred turns later, in the same function. **A
partially ported function does not announce the part that is missing**, and the part that
was missing here was the only one a player could see.

---

**C188 — the audio layer had no `stop`, and *"blocked on finding a caller"* was
blocked on nothing.**

Two player reports on `EE0CB9233`, and one root each.

**"VO doesn't seem to stop when the dialogue that produces it is closed, eg tutorial it
will finish the line."** `Msg_Dismiss` (`0x00476768`) is four statements of state and one
that is not: `if (g_messageGroup != 0xc2) Sound_StopOneShot();`. `Opt_ToggleMusic`
(`0x004349A4`) has the same call on the arm that turns music *off*, so the Music row
silences speech as well. `Audio::stop_one_shot` had been written, documented against its
address, and **called from nowhere in the workspace** — the verb existed and the two sites
did not. The inventory could not see it: `tools/oracle/sounds.js` scans for the nine
*play* primitives, so all sixteen `Sound_StopOneShot` sites are outside the census by
construction, and `docs/audio.json` was green with every voice line `reproduced`. **A
census of what the game asks for cannot tell you the answer goes on too long.**
`docs/audio-triggers.md` now says so in the section that excludes them.

Group `0xC2` — `L2.eng` 194, *"Foiled again."* — is the original's one exemption and is
reproduced with it; the ablation that drops the guard is red.

**"No VO for 'A band of scottish pikemen are available for hire, my lord' with a
mercenary."** `docs/audio.json` filed eight sibling voice thunks `blocked`, all eight
carrying one note: *each is one `Sound_PlayFile` behind one table index, so the work is
finding the caller rather than the sound*. **Seven of the eight callers were one grep of
`tools/oracle/decomp/` away**, and had been for as long as the corpus has existed. The
note was written once and copied to eight records, and the copy is what made it look
answered: nobody re-reads a sentence they have already read seven times.

Three of them are now built — `Sidebar_Button` hotspot 1's mercenary offer
(`FUN_004B3714`, `S016_01` … `S016_12`, and band 1 is the Scottish pikemen exactly),
`Panel_OpenPopulation`'s health line (`FUN_004B3768`), and the map information panel's
sentence about whatever it opened on (`FUN_004B37BC`, both openers of screen `0x04`).
`reproduced` 86 → **89** of 143; `blocked` 23 → **19**; the non-voice files this engine can
reach, 30 → **55**, and the install total this engine can reach, 678 → **703** of 771.

**Three things the reading found that building would not have.**

* **`g_screenId = 0x17` has two writers and only one speaks.** `Armoury_Button`
  (`0x00435AE8`) id 2, *Change*, opens the same screen silently. Ours reaches it by
  `Transition::Replace`, so the previous tick's stack is what tells the two arrivals apart
  — without that gate a player toggling Change and Continue hears the band announced once
  a second. Asserted both ways.
* **The tables are not conventions.** `0x004E2058` reads `S020_01, _02, _03, _04, _04,
  _05, …` — entries 3 and 4 are the *same file*. `healthBand` is `0 ..= 4`, so a
  `format!("S020_{:02}", band + 1)` would have spoken `S020_05.wav` — a file that ships —
  for the healthiest county in the game, and nothing would have gone red. It follows that
  **`S020_05.wav` is named in the binary and unreachable in play**, `battle5.wav`'s
  situation. `S246`'s table is out of order too (`_02, _04, _03, _01`). Every table here is
  transcribed from `.data`.
* **The eighth thunk has no caller, and that is a finding about the original.**
  `FUN_004B39E8` plays the degraded castle's three lines, `S075_02`/`_03`/`_04`, all of
  which ship. Zero `E8` rel32 calls, zero `E9` jumps and zero dword references anywhere in
  the image reach it; its sibling `FUN_004B3714` scores exactly one (inside
  `Sidebar_Button`) under the same scan. `docs/bugs.md` **D40**, `docs/audio.json` `dead`,
  and the fourth entry in `tests/sfx.rs`'s pinned set. **"Blocked on finding a caller" and
  "there is no caller" are different answers to the same question, and only one of them is
  reachable by building.**

**What stays blocked, with the real reason rather than the copied one.** `FUN_004B36C0`
(`Ui_OpenConfirm`, `S010_*`) — the caller is known and we have one of its thirteen call
sites, the battlefield's autocalc, which is a field on `BattlefieldScreen` and not a
`ScreenId`; a confirm box that is a screen unlocks all thirteen at once. `FUN_004B3940`
(`CastleBuild_Select`, `S071_*`) — the only one of the eight whose trigger is a click
*inside* a screen, and the selection is that screen's own field. `FUN_004B3994` (`S035_*`)
— screen `0x20`, a front-end page not in `ScreenId` at all. `FUN_004B3B92`
(`Msg_DrawWindow` ×4, the lord sting) — needs `voice_tick` to return a list, unchanged.

---

**C189 — three screens each drew a comment instead of a field, and all three
comments had been true when they were written.**

Three unrelated player reports, one shape. Every one of them is a *stale premise* left in the
source as a sentence, still read as a fact after the thing it denied had been built.

**1. *"When I attacked and it asked me to decide it said I had 0 men, I think it's because it
was just mercenaries."*** `FUN_004224E7`, the roster painter screens `0x12` and `0x13` share,
ends both of its modes with `Ui_DrawCount(g_units[p].menTotal, 0x48, …)` — the unit record's
`+0x168` — and folds the band into its own row on every one of its seven rows,
`if (g_units[p].mercTroop == local_c) local_10 += g_units[p].mercMen`. It has to, because
`Mercenary_Hire` (`0x004AC7F3`) adds the band's men to `menTotal` and **never touches `+0x16C`**:

```c
g_units[unit].mercMen  = (&DAT_00568dc8)[band * 0x14];
g_units[unit].menTotal = g_units[unit].menTotal + *(int *)(&DAT_00568dc8 + band * 0x14);
```

So an army raised with nothing but a hired band carries seven zero counts and a real total, and
the two numbers are *not* interchangeable. Ours summed the seven counts for the total and passed
`Unit::troops` raw for the rows. The right number was already on the question —
`turn::Question::attacker_men`, read from `u.men` — and was **never drawn by anything**. Fixed by
passing the two totals into `draw_roster` and by folding the band in one named place,
`engagement::roster_of`, which both the question and the report now read.
`tests/military.rs` `a_mercenary_only_army_is_not_drawn_as_no_men_at_all` asserts the two ways of
holding two hundred pikemen paint the identical prompt.

**2. The box at the far zoom was empty, and C173 is the correction that emptied it.** C173's own
closing note filed this and did not close it. `Screen_DrawCampaign`'s (`0x0040F5FD`) zoom-2 arm:

```c
Ui_DrawBox(0, 0x19C, 0x1E, 4);
DAT_0058FE2C = 1;  g_penAdvance = 0;
Eng_DrawString(0x65, g_scenarioIndex, 0x40, 0x1A8, &g_fontHeading, 0x3F);
Eng_DrawString(0x22, 0,  g_penAdvance + 0x50, 0x1A8, &g_fontHeading, 0x3F);
Ui_DrawYear(g_year,      g_penAdvance + 0x60, 0x1A8, 1);
DAT_0058FE2C = 0;
Eng_DrawString(0x22, 1, 0x50, 0x1C6, &g_fontBody, 0x3F);
```

Drawn, from the player's own file, with our transcription only as the fallback. **Group 34 has
exactly one consumer in the whole binary and it is this arm**, which is rule 6's test, not a
naming lead: its two strings *are* this box's vocabulary, and the second of them — *"Click on
the county you wish to view."* — is the game saying what the far zoom is for.

**And the correction that filed it described it wrong.** C173's note and `docs/screens.md` §7
both say the box wants *"the map's name, the season and the year"*. **There is no season in this
box.** The painter makes three heading draws and a season is not one of them; the season is on
the menu bar, out of group 29. A hole filed from prose rather than from the listing carries the
prose's errors forward — this one would have had an agent hunting for a season draw that does
not exist.

**3. *"It says Court of LORD1 even though I wrote a name when I started the game."*** `LORD1` is
not in `L2.eng`. A scan of all 317 groups for a lord-plus-digit returns nothing, and group 7 —
the only default-name group there is — holds *"No player"*, *"The Knight"*, *"The Baron"*, *"The
Countess"*, *"The Bishop"*, indexed by the **lord** and not by the realm. So it was not a
fallback showing through: it was ours. **One defect, not two** — the typed name reaches
`Game::player_names` correctly (`Player_SetHuman`, `0x0049BAE9`, and `tests/text.rs`
`start_puts_the_typed_name_into_the_realm` has asserted it all along) and `court.rs` ignored it,
under this comment:

> The original draws `g_playerNames + realm * 0x2C` here; **nothing in this tree carries them
> yet** and `screens/battle.rs` has the same hole.

True when written. False from the moment hand-off 2 existed, and the comment is why nobody
looked again. `screens/battle.rs` had the same line for the same reason and is fixed with it;
both now go through `message::lord_name`, which is `g_playerNames` with group 7 behind it — the
original's own two sources, in the original's own order.

**What the three have in common is the thing worth keeping.** `tests/text.rs` enumerates four
hand-offs a typed name has to survive and tests each one; all four were green while the court
drew `LORD1`, because the fifth — *the array onto a screen that is about the player* — was not on
the list. A chain of hand-offs is only as long as somebody wrote down, and the end of the list is
not the end of the chain. The same is true of the battle prompt: `attacker_men` existed, was
correct, was covered by a test that read the field, and **no test read the pixels**.

---

**C190 — a screen id is a destination, and we had no way
to say one. Two player reports, one missing idea.**

Two defects arrived as separate reports and share a shape: in both, the original states
where to *go* and ours could only say where to *come back from*.

> *"There doesn't seem to be a last-turn autosave either so I can't easily repro that for
> you."*

`Save_RotateAndWrite` (`0x0049A453`) keeps **three** turns, not one, and its names are three
13-byte literals — `lastturn.sav`, `old_turn.sav`, `safeturn.sav`, `[V]` out of `.rdata` at
`0x004DC2F0`. The rotation is `remove(safeturn); rename(old_turn, safeturn);
rename(lastturn, old_turn);` and then `Save_Write(lastturn)`, every step's failure ignored —
which is the whole reason a game's first autosave works. `docs/environment.md` has recorded
the *file name* for weeks and nothing had read the function; three deep is the part that
matters to the report, because the turn a player wants is often the one before the one that
went wrong.

**Where it is written was the second finding.** Exactly two callers, `[V]`: `Game_NewGame`
(`0x00497E2B`) and `FUN_0049A3E6` (`0x0049A449`), now `Season_FinishFade`. The latter fires
on `g_screenId == 0x24` — the bottom of the end-of-turn fade — and its statements are
*reload the seasonal art, repaint, present, fade back up, autosave*. So the file holds the
**opening of the turn that just began**, not the end of the one that finished, and
`l2_view::fade::is_darkest` already named that frame for the art reload without anyone
noticing the line beside it.

> *"After a castle film the player is stranded on the chooser under a tip."*

`CastleBuild_Confirm` (`0x00436B59`) calls `Smk_Play(castle1.smk + level * 0x10, 0x9E, 0x14,
0, 0)`. **The fifth argument is a literal `0`: the campaign map.** Reading all eight
`Smk_Play` call sites is what makes that mean something — seven pass `g_screenId` itself or
the front end's `0x1F`, which in a stack is *come back where you were*, and exactly one names
a screen that is not the one it was raised over. Ours popped the film and left the chooser to
pop itself on its next `update`; `Machine::update` runs `run_tips` and `pump_messages` first
and `tip::DELAY` is `0x14` frames, so on any film longer than twenty the castle advisor tip
seated itself and the chooser never got that update. The ablation reproduces the report
exactly: `[Campaign, Castle(1), Tip, Message]`.

`Transition::Goto(ScreenId)` is the idea that was missing — *go to screen X, unwinding the
stack*. It is neither `Pop`, which only knows what it is leaving, nor `Replace`, which leaves
everything underneath standing. The original needed no such vocabulary because `g_screenId`
is one byte, and a codebase that models one byte as a stack has to say the difference out
loud or it cannot say it at all.

**The autosave is raised, not performed**, and that is the part worth keeping. Seventeen test
files end a turn through the map screen; if `tick_fade` wrote a file, every one of them would
rotate the developer's own autosaves out from under them on every run — which is the exact
defect being fixed, committed by the fix. So a screen reports it the way it already reports a
widget click (`Screen::take_autosave`), the machine drains it, and `saves::run_pending` is the
only thing in the workspace that turns it into a file. Nothing of it is on `Game`, so it is
in neither the save nor the lockstep digest.

**What was left alone.** `Film::Ending { game_over: true }` stays a `Replace(Conquest)`: its
return screen is `g_screenId` *read after `Msg_Dismiss` has already entered `0x1C`*, so there
is no screen on the stack to unwind to and "become this" is what the byte is saying. And
`DAT_00553260`, the guard that suppresses the rotation, is not built — it is raised only by
`FUN_0049B973`, the multiplayer com-link-error resync, and we have no network game.

---

**C191 — the only control on any job popup, and the sentence that
says it is one. Both were filed as missing, and the *table* was filed without half of its
own call.**

A player: *"I can't choose what type of weapon my blacksmiths are making."*

**Everything the choice does to the simulation was already here.** `County::weapon_type`
(`+0x290`) has been a field since C153; `industry::weapon_shares` is `Industry_WeaponShares`
(`0x0044F15B`); `WEAPON_COST` is `g_weaponCost` (`0x004D8990`); the AI's rota writes the byte
every season. What did not exist was **a way for a person to write it**, and there is exactly
one in the original: six hotspots over the smithy picture, `Hotspot_Test(0, 0x18,
&DAT_004DCA10, 6)`, dispatched from `Screen_HandleInput` (`0x004BA9C8`) and not from
`Screen_FrameInput`. That placement is why C61's denominator note exists: an enumeration of
the dispatcher scores this zero without it ever appearing as a miss.

**The record in `docs/arms.json` named the table and not the call, and that is the finding.**
`Hotspot_Test`'s first two arguments are **offsets added to every record before the test**,
and the call passes `(0, 0x18)` — which is exactly where `Sprite_WGenSprite(0, 0, 0x18)` puts
`Smithy.pl8`. So the six rectangles are in the **picture's** coordinates. Read as screen
coordinates, as a careful person reading only the table would read them, every weapon sits
twenty-four pixels high: a click a player aims at the pike lands on the bow, and it lands
*silently*, because the county still changes what it forges. `crates/l2-game/tests/job_bodies.rs`
asserts both corners of all six against the player's own exe so that reading cannot recur.
It is the same shape as `docs/arms.json`'s gesture field (C148): the record carried the
address that would have answered the question and nobody read the rest of the line.

**`FUN_0043A997` (`0x0043A997`) is five statements and four of them are the recompute**, now
`l2_kingdom::Kingdom::set_weapon_type`:

```c
county[+0x290] = weaponType;
Industry_LabourEstimate(county, 2, 7, 0xF, 4);
Labour_Allocate(county);
County_RefreshEstimates(county, g_seasonNext);
FUN_00448648(owner);
```

The order of the middle three is load-bearing and is **the opposite way round from
`Industry_ToggleFromMap`**: the estimate writes `labour_useful[7]`, the ceiling, and the
allocator deals against it, so a cheaper weapon takes more smiths *on the click*. Deleting
the leading estimate leaves the ceiling right — the two refreshes below it put it back — and
the **headcount** wrong, which is why the test reads the headcount. And the closing
`FUN_00448648` is the realm-wide half: a blacksmith's ceiling is a share of the realm's
stockpile split across every staffed smithy it owns, so **what one county forges changes what
another one can**. That is the Readme's *"turning a blacksmith on will reduce the resources
available to other blacksmiths"* seen from the other side, and England turn one cannot test
it at all — every realm there owns exactly one county, so the fixture is `siege-lastturn.sav`,
whose realm 1 holds three.

**Rule 6 again, and this time the group is a group of one.** `Panel_JobBlacksmith`
(`0x00413155`) is `L2.eng` **group 75's only consumer in the whole binary**, and index 0 is
*"Click on a weapon to change production."* — the sentence that tells a player the picture is
a control. A page of six invisible hotspots with that line missing is not a page with a
cosmetic gap; it is a control nobody can find, which is what the report was. Indices 1 and 2,
*"wood needed."* and *"iron needed."*, are in the file and **on no screen**: the two cost
figures pass `&DAT_004D3E9C` and `&DAT_004D3EA0` as their suffixes and both are the empty
string, so the iron bar and the log carry the meaning instead. Transcribed anyway, because
the group is the page's specification.

**What moves in the lockstep digest.** `County::weapon_type` is already inside
`l2_kingdom::save`'s canonical encoding, so nothing new is hashed — but the setter now moves
it, and with it `labour[7]`, `labour_wanted[7]`, `labour_useful[7]` and
`industry[2].next_season` **on every county of the acting realm**. No season pass changed and
the End Turn differential does not move: this is a player command, and it is a command in the
lockstep sense too — the original's own multiplayer arm is `Net_SendCommand(0x21, 0)` with
`FUN_0043A997` run on every machine, which is this crate's model already.

**Two smaller things went in beside it and one was a five-line stub of ours.** The job icon —
`Sprite_WGenSprite(DAT_004D2974[job], 0x41, 0x69)` out of `Iconvill.pl8`, whose job-9 frame is
overridden to `0x10` by the painter itself — was the last of the nine panels' missing draws,
and the recess had been drawn empty since C177. And `text::draw("THE SMITHY IS A FULL PAGE")`
is gone: `tools/draws/screens.json` had it under `literals_ours` and the draw audit's caption
test is what noticed, which is the inventory doing the job it was built for rather than a
person remembering.

---

**C192 — The battle overview panel was black, and the schedule that fills it is not
the one a single function shows.**

The player, on build `EE0CB9233`: *"battle is still a blue mess where the grass should be, a
black minimap"*. The blue was C183's palette. The panel was a painter we did not have: ours was
`fill_rect(ink.background)` — `ink.background` is the index nearest black in **every** palette,
so no palette work could ever have touched it — with a dot a *side* on it and a rectangle round
the camera.

**What the original draws.** `Overview_DrawRows` (`0x004BC51A`), registered by
`Battle_LoadAssets`' `Overview_SetSheets(t2_bat1.pl8, t2_bat2.pl8, t2_spri.pl8, 0x1E0, 0x18, 2)`
— entries `0x0B`, `0x0C` and `0x11` of the asset table at `0x004DA550`. Two pixels a cell from
`(480, 24)`, so the 80 × 80 field is a 160 × 160 raster ending exactly where
`Screen_DrawBattlefield` puts `Misc_bat.pl8` frame 0, at `(0x1E0, 0xB8)`. Per cell: frame
`cell[+3]` of `t2_bat1.pl8` — **252 frames of 2 × 2, one for each of `T32_bat1.pl8`'s 252
32 × 32 tiles**, so the same `gfx` byte indexes both — or, over a cell holding a man, frame
`g_realms[owner].shieldIndex` of `t2_spri.pl8`, which is seven 2 × 2 frames: an erase tile and
six flat colours. **The men are coloured by realm, not by side**, and a `shieldIndex` of 0 draws
no man at all. **There is no viewport rectangle.** `[V]`, and the file headers agree: 252 and 7,
all 2 × 2. **C183's trace of this, written without building it, was right in every particular
it stated** — the sheets, the 2 × 2 tiles, the shield colour, the absent rectangle, the four
rows. Re-derived here from the decompilation because it was a trace and not a finding, and what
it did not state is the two paragraphs below: how often the *full* pass runs, and that
`t2_spri.pl8` frame 0 is an erase tile on the same sheet as the men.

**Two dead branches, named so nobody builds them.** `flags & 0x1C == 4` reaches the second tile
sheet, which for a field battle is `t2_bat2.pl8` — size `0` in the table, absent from the
install, and skipped outright by the loader's non-siege arm. And column 0 draws the erase tile
when `g_appPhase == 3`, which is a start-up phase; `App_Draw` has taken it past 8 long before a
battle.

**The schedule is the part that cannot be read off one function.** `Overview_Step`
(`0x004BC1D1`) advances a row cursor, wraps it at `0x50 − n`, and paints `n` rows.
`Screen_DrawBattlefield` enters the screen with `g_mapRedraw = 1` and `Overview_Step(0x50)`, and
`Battle_Frame` then runs, once a frame while `g_battlePhase == 2` and `0x27 < g_screenId < 0x2B`:

```c
if (g_mapRedraw == 0) { Overview_Step(4);    Gfx_MarkSpriteDirty(0x1E0, 0x18, 10, 10, 1); }
else                  { Overview_Step(0x50); Gfx_MarkAllDirty(); }
FUN_004BC142(cameraX, cameraY);
```

Read that far and the honest reading is *"the full pass runs whenever `g_mapRedraw` is set, and
who knows how often that is"*. **`FUN_004BC142` ends `if (g_mapRedraw != 0) g_mapRedraw--`** —
so the full pass happens on the frame after entry and never again, and the rest of the battle is
**four rows a frame: a twenty-frame sweep**. `[V]`, and it took the callee to settle it. **It is
visible.** Eighty rows at four a frame means a man's dot appears at his new cell up to twenty
frames after he is there and vanishes from his old one just as late, so the panel and the
viewport disagree about where he is for as long as he walks. **How long that is in wall-clock
time is not claimed** — nothing here measured the original's frame rate. It is reproduced, and
`the_overview_panel_is_repainted_four_cell_rows_a_frame` asserts
that exactly four cell rows and no more come back into agreement per frame, and that twenty
frames sweep the panel.

**Why ours keeps a raster of its own.** The original paints into the back buffer and the
seventy-six rows it did not visit keep the pixels they already had. Our canvas has a yes/no box
and an outcome film pushed over it and the original's screen has neither, so `BattlefieldScreen`
holds the 160 × 160 raster and blits it whole. The schedule — which rows, in which order, how
stale — is the original's.

**And a screen nobody has: `Overview_SetSheets` has a second caller.** `FUN_00498DCB` registers
the same painter at `(0x1D2, 0x0B)` over the `t2x*` sheets with `Misc_ske.PL8` beside it, and it
is called by `Skirmish_Setup` (`0x0042B7F7`) and `Screen_BattleMasterRatings` (`0x00421707`).
All seven of those files ship. Skirmish is `missing` in `docs/features.json` and this says what
its sidebar is made of when somebody builds it. Found, not built.

**Left alone: cell byte `+2`.** The original's per-cell dirty bits (`flags & 3`) and its sheet
selector (`flags & 0x1C`) live in a byte `l2_sim::terrain::Cell` does not carry — it keeps `+0`,
`+1`, `+3`, `+4` and `+7`, which is what a `.skr` field battle writes. Repainting every cell of
the four rows visited is what `g_mapRedraw` makes the original do anyway, and the erase tile is
one pass of a cell the next pass draws terrain on. Modelling `+2` would be a simulation change
for no picture.

---

**C193 — the film ran on a tick, and the tick was two percent slow.**

> *"The sound seems to be a bit desynced from the video"* — in ours, not the original.

**What paces a film in the original, `[V]` on both halves.** `App_WinMain`'s message pump
(`0x0040E9AB`) has **no throttle at all**: `PeekMessageA`, and when the queue is empty
`App_IdleFrame` (`0x0040E8BB`) → `App_Draw` → `Battle_Frame`, straight round again. Its only
`Sleep` is the 200 ms one taken when the window is inactive. So the original has no tick and
no frame rate; it draws as fast as the machine allows, and anything paced reads a clock for
itself. `Smk_PlayLoop` (`0x0042DBC7`) does exactly that — `if (SmackWait(g_smack) == 0) {
…decode, blit, SmackNextFrame… }`, a poll that does nothing until the frame is due — so
**the whole of the film's clock is inside `smackw32`**, and the question *"audio buffer,
header rate, or the game's tick?"* is answered for the game's side before the DLL is opened:
not the tick, because there is no tick.

**And inside the DLL.** `_SmackWait@4` is 320 bytes at RVA `0x3170` of `Smackw32.dll`, and
the only imported function it calls is `WINMM.dll!timeGetTime`, at `+0xB0` — `[V]`, by
matching every `FF 15 <imm32>` in `BEGTEXT` against the import table and attributing each to
the export it falls in. So the deadline is in real milliseconds. Whether the sound driver
slews that deadline — the DirectSound path installs a `timeSetEvent` callback,
`_TimerFunc@20`, which also reads `timeGetTime` — is not decidable from the call sites, and
**it does not need to be**: measured over the install's 45 films, every sound track runs
`frames × period` long to within **1 ms**, on films as long as 131 s. The audio buffer and
the header's rate are the same clock. `crates/l2-smk/tests/corpus.rs`,
`every_track_is_as_long_as_its_picture`.

**Ours was neither.** `movie::Player` counts ticks of `TICK_MS` and converts, which is right
if a tick is 16 ms. The event loop said:

```rust
let now = Instant::now();
if now < self.next_tick { … return; }
self.next_tick = now + TICK;        // measured from *after* the wait
```

`now` there is the deadline plus however far the wait overshot, and the next deadline is
measured from it, so **the overshoot is kept rather than repaid and compounds once per
tick**. It can only ever run slow: overshoot is never negative. Measured with winit 0.30's
own wait primitive — `CreateWaitableTimerExW` with `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`
and `WaitForSingleObject`, which is what `ControlFlow::WaitUntil` runs on Windows — a 16 ms
tick came out at **16.31–16.42 ms**, +1.9 % to +2.6 %, with no work in the loop at all.

**So the drift grows, and it is a rate rather than an amount.** 2.4 % of `intro.smk`'s
131.5 s is over three seconds of picture behind sound by the end; 2.4 % of `bat_win5.smk`'s
3.6 s is 71 ms, under one of its own frames. That is why the complaint is about the long
films and why nobody saw it on a battle result.

**The fix is `clock::Ticker`**: every deadline measured from the deadline before it, so an
overshoot is repaid on the next tick, with a `MAX_CATCH_UP` of eight ticks past which a
stall is written off rather than fast-forwarded. It is in the library and not in `main.rs`
for the reason `audio::Director::listen` and `saves::run_pending` are — a binary's code
cannot be called by a test — and it reads no clock itself: it is integer arithmetic over a
monotonic reading the caller supplies, so `docs/netcode.md` D-12 is untouched.

**This was never only about films.** Every animation, press timer and tip in the game is
counted in ticks and every one of them was running two percent slow; the film is simply the
only one with an independent clock beside it to disagree with.

**What was measured and left alone.** Two constant offsets remain, and both are under a
quarter of a frame: a frame is shown at the first tick *at or after* it is due, which at a
16 ms poll is a mean 8 ms late where `Smk_PlayLoop`'s poll is microseconds; and a film's
track starts on the tick its screen reaches the stack while frame 0 is decoded on the next,
one tick later. Neither compounds, and closing either means either showing frames early or
opening the film inside the transition that pushes it.

---

**C194 — the standings page's own inventory row said the marker was
the leader's, and it is the category's. A dead button, and the two documents that
described the screen both described it wrong.**

> *"I can't click the Greatest Nobles button in the treasury view."*

The button was not the defect. `court.rs` answered it with `Transition::Stay` and said so
in a comment — *"`0x20` is not built, so this is the arm and not the destination"* — which
is the honest form of a gap and is still a live-looking button with nothing behind it.
`docs/arms.json` had it on file as `missing` and `docs/features.json` had the screen on
file as `missing`, so nothing was hidden; what was missing was the screen.

**Two records described screen `0x20` before it was built, and both were wrong in the same
place.** `docs/draws.md`'s inventory row said `flags.pl8` frame 5 was *"for the leader"*.
`docs/symbols.json` said the painter *"walks the realms at stride 0x160 and labels each
from `g_playerNames`"*. The painter draws **one** name, not five, and frame 5 is indexed by
`g_nobleTabX[g_nobleCategory]` — the **tab you are looking at**, not the realm that is
ahead. The file settles it without reading a line of code: frames 0…4 of `Flags.pl8` are
51 × 92 and frame 5 is **23 × 60**. A sixth banner would be 51 × 92.

Neither sentence was careless. Both were written by someone reading the painter *for a
count* — how many draw calls, which sheet — and a draw-call audit has no column for *what
is this frame of*. `CLAUDE.md`'s note on `[V]` is about exactly this shape: a true
statement about one question, promoted to an answer to another. What makes it worth a
number is that the audit's own note called seven draw calls for a standings table
*"suspiciously few"* and resolved the surprise the wrong way — the denominator was right
and the **noun** was wrong. It is not a table. It is a bar chart of five flagpoles.

**What the page actually is.** `Screen_GreatestNoble` (`0x0041593B`) draws one banner per
in-play realm, at `(g_nobleColumnX[slot], (100 - pct) * 2 + 0x2D)`, on a four-pixel pole
running from the banner's bottom edge down to `y 0x158`. `g_nobleColumnRealm`
(`0x004D2B58`) is `5, 3, 1, 2, 4`: **the seating order is not the realm order** and realm 1
stands in the middle. The pole is four `FUN_00403A8F` line draws, which is not one of the
audit's 26 primitives, which is why a chart of up to twenty lines counted as zero.

**`FUN_00415E42`'s seven scoring rules were written down nowhere**, and two of them are
behaviour rather than plumbing. Category 1, *Most castles*, reads realm `+0x4C` **as a
byte**. Category 6, *Greatest noble* — the overall standing, the thing the screen is named
after — returns a **flat 2 for every realm while `g_year < 0x4F6`**, so it reads
*"Greatest noble, undecided."* until 1270 in every game ever played. Neither is
discoverable from the painter; both are one `if` in a function the draw audit never
opened, because it makes no draw calls.

**The bars are a percentage of the leader, and a tie is not a lead.** `FUN_00415BDC` writes
`PctOf(score, best)` clamped to 0…100, so the leader's pole is always full height; sets
`g_nobleAllLevel` when every in-play realm scores the same, and then overwrites every bar
with **50**, the one number the page draws that it did not compute; and otherwise sets
`g_nobleTiedAtTop` when anybody else matches the leader. The name is printed only when
neither flag is set. The leader itself is chosen by `if (best <= v)` — **`<=`, so a tie
goes to the highest realm index** — which never shows, because the same function then calls
the category undecided. It is reproduced anyway: it is what would be printed the day either
flag stops being set, and a leader chosen the other way would be a different lord.

**The sound was `blocked` for a reason that turned out to be a choice.**
`docs/audio.json` files the castle chooser's five spoken names as blocked because *"the
selection is `CastleScreen`'s own field rather than anything on the `Game`, so a click that
changes it is invisible to the Director"*, and it filed `S035` the same way. But
`DAT_0055CE7C` **is a global in the original** — `Game_NewGame` zeroes it and it outlives
the page being closed — so keeping it on `Game::nobles_category` is the faithful placement
and the one that unblocks the sound. The edge is a counter, not a diff: `FUN_004B3994` has
exactly two callers, `Court_OpenGreatestNoble` and `GreatestNoble_SelectCategory`, and
**both call it unconditionally**, so pressing the tab that is already showing speaks again
and a diff on the category would swallow that press.

Two entries of the voice table are past what either caller can reach, and they are
different mistakes: `S035_08.wav` — the line for *"undecided."* — ships and is played by
nothing, and `FUN_004B3994`'s guard admits **nine** where the table at `0x004E2168` holds
eight, so index 8 would read `S075_01.wav` out of the next table along.

**What was left alone.** `Ui_OkButtonClicked` on this page sets `g_screenId = 0` — the map,
not the court it was opened from — and ours pops onto the court. That is `C190`'s
`Transition::Goto` and it is deliberately not used here: the original throws the court away
because one byte has nowhere to keep it, and the court is where the button was.
`docs/arms.json` `0x0042FF10/standings-ok` carries the reasoning. The multiplayer arm,
`Net_SendCommand(0x3B, 0)`, is not built; there is no network game, and the single-player
arm is what `Game::multiplayer` being false already means everywhere else.

---

**C195 — C124 fixed the wheat by reading one of `Grain_SeasonTick`'s three
band calls, and wrote it down as all three.**

> *"wheat fields still not showing the different stages of wheat growth."*

**"Still."** C124 found `Terrain_Set`'s live variant and built both halves — the repaint and
the frame term — and a test at the pixel. The player's build carried all of it. What it
carried was this, in `maps-layers.md` §5.5, in C124 above, in `campaign::field_variant`'s
doc and in `land::grain_repaint_fields`:

```c
band = FUN_0044CF6F(county.crop[2], county.fieldsGrain);
```

`Grain_SeasonTick` (`0x0044C8AE`) calls `FUN_0044CF6F` **three times, once in each arm**, and
that line is neither argument of any of them. **[V]**, all three call sites:

| `g_season` | the arm | the band reads |
|---|---|---|
| 1, Spring | `Grain_Sow`, weather cut, `crop[1] = crop[0] * 12` | `FUN_0044CF6F(crop[1], (byte)+0x206)` |
| 2, 3, Summer and Autumn | `Grain_Grow`, weather | `FUN_0044CF6F(crop[1], (byte)+0x206)` |
| 4, Winter | `Grain_Harvest`, weather | `FUN_0044CF6F(crop[2], (byte)+0x206)` |

`crop[2]` is cleared at the top of every season and refilled only by the harvest arm. So the
transcribed line banded a **zero** for three seasons in four, `FUN_0044CF6F` returns `2` for a
zero crop, and every grain field on the map drew variant 0 — the sparsest of the four crops —
from sowing until the harvest. The C124 test could not see it: it wrote the terrain byte by
hand and asserted what the painter made of it, and nothing asked what a season writes there.

**And the divisor is a field this project had filed as a duplicate.** `+0x206` is written
beside `+0x202` by the sowing arm — `fieldsGrain`, or `1` on a shortfall — and
`docs/stored-fields.json` excluded it as *"a second copy of +0x202 that only the tile graphic
reads"*. **Three functions read it and two write it.** `County_DestroyField` (`0x00469E5B`)
and `FUN_0046965A` step it down whenever `fieldsGrain <= +0x206`, and nothing steps `+0x202`
down; `County_DestroyField` charges its crop share against it, with an `else 0` when more
grain is painted than was sown. It is not derivable from the other two counts — destroy a
field and then paint two, and the orders disagree — so it is a county field now,
`County::fields_grain_standing`, imported, and in the save at version 25. **That `why` was
the same shape as C124's `[V]`: a true statement about one reader, promoted to a statement
about the field.**

**What a wheat field passes through, all `[V]` against the decompilation.** Frames are
`Terrain_Set`'s, `+ (stored & 3)` for the tile's own variation; `Roads1a … d.pl8` by season at
the near zoom (§1.1c), `Roads2a.pl8` in every season at the far one (§1.1b):

| terrain | frame | written by, and when |
|---|---|---|
| `0` wild | 80 | `Map_PlaceStartingFields` at setup; the brush's *abandon*; `County_DestroyField` when an army tramples it; `FUN_0046942C` at the next weather pass after a flood or drought |
| `0x19 … 0x1C` reclaiming | 108, 112, 116, 120 | the brush starts it; `Field_ReclaimTick` (`0x0044C093`) at 200/400/600/800 progress, then `1` |
| `1` fallow | 84 | setup; the brush; the end of reclamation |
| `2` sown, variant 0 | 88 | the brush; every season's band for a crop under one sack a field, or zero |
| `3`, `7`, `0xB`, variants 1–3 | 92, 96, 100 | `Grain_SeasonTick` → `FUN_00469D21`, **every season**, at under 41, under 81, and 81 sacks or more a field |
| `0x17` flooded, `0x18` parched | 130, 134, *base* bank | `Weather_UpdateAll` (`0x00449889`) → `FUN_00469A9C`, one field a county in weather 5 or 1 |

**There is no harvested stage.** The harvest repaints the band like any other season, from
what was reaped; the field stays grain through Winter and the next Spring sows over it. And
**the four crops are densities, not a growth timeline**: a county that sows 60 sacks a field
and tends it fully draws variant 2 in all four seasons, in the original as in ours. What
changes the picture through a year is the *seasonal sheet* the whole roads bank is swapped to,
and the band moving when the labour cap, fertility, the weather, a trampled field or a thin
harvest moves the crop per field.

**Four more things, found on the way.**

* **`kingdom.rs` called the repaint twice**, the second under a copy of the first's comment
  with both of its names stripped out. The repaint is idempotent, so it changed no pixel.
* **`County_DestroyField`'s repaint is unconditional** and ours returned early from both arms
  when the guard failed, leaving the tile standing. Ported now, with the `+0x206` arm.
* **The flood and drought damage is not built.** `Weather_UpdateAll` repaints one field a
  county to `0x17`/`0x18` through a round-robin cursor at `+0x15B` bounded by `+0x205`, before
  its *Advanced Farming* override — and `FUN_0046942C` turns every such field to **wild** the
  next season, so a flood costs a field for good. `l2_kingdom::weather::update_all` does
  neither. It is a simulation rule with saved state of its own (the cursor), outside a picture
  fix; recorded rather than built.
* **The England turn-one county's granary is empty**, so on that position nothing can be sown
  until seed is bought, and a field painted *before* the seed arrives gets no farmers until
  the next estimate round — `Field_SetType`'s own `Labour_Allocate` sizes the grain ceiling
  from the store. Both are the original's.

**The test is a year, through the screens.** `crates/l2-game/tests/wheat.rs` sows with the
brush's handler, presses End Turn four times through the machine with the fog on, and after
each turn paints the campaign map and requires the inner diamond of a lit field to equal the
terrain pass drawn with the **literal** frame `0x58 + (stored & 3) + 4v` — `v` transcribed from
the three calls above, not asked of `l2_kingdom` — and to differ from the other three, at both
zooms. Our field markers are over the selected county's tiles and cover a far-zoom tile
entirely; the test looks with another county selected, and says why.

**Ablated line by line**, and three of the ablations left the screen test green. Banding
`crop[2]` outside Winter, dropping the repaint call, dropping the variant term and fogging
the field each turn it red. Dividing by `fieldsGrain`, reading `crop[1]` in Winter and
deleting the shortfall arm do not: in that year no field is lost after sowing, the harvest
equals the standing crop and the seed covers a sack a field, so the screen cannot tell those
lines from their ablations. Each is pinned by a unit test in `l2_kingdom::land` that goes red.
**One more green was the test's own fault**: it first read `+0x206` back from the county to
compute what to expect, so deleting the write moved the expectation with the picture. It
pins the divisor from what it sowed now, and that ablation is red.

**The band is not only a picture, and one test went red for it.** `Ai_FindStandingCropTile`
(`0x004A689D`), the tile an AI raiding party (mission 7) marches at, accepts a plane-0 `0x20`
tile only when `2 < content && content < 0x17` **[V]**, the literal, and `ai_army::Aim::StandingCrop`
matches it. Terrain `2` is excluded. Under C124's reading every sown field sat at `2` for three
seasons in four, so **the AI's raids could not see a crop for most of the year** and fell back
to the county anchor. With the band right they find the harvest, which is the binary's rule.

**And that is why the branch sat unmerged.**
`l2-game/tests/ai_war.rs::the_ai_realms_survive_forty_turns_of_a_world_built_by_hand` was red
on it: realm 4 ended forty turns on one county with 2 people at health 0. The branch measured
three ablations — C124's band back → green; the raid finder blinded → green, realm 4 at 183
people and health 57; the old `destroy_field` back → still red — and left the question open:
is *"every surviving AI realm is fed"* a rule, or one trajectory's luck?

**It is one trajectory's luck, and re-measuring on `main` is what shows it.** Battles, the
clock, labour and the herd's break-even floor landed between `fb5c0ef` and this merge, and on
the trajectory they produce the test is **green with the fix in**: realm 4 is *conquered* at
turn 21 while at health 71 with 375 people, not starved, so assertion 2 — which only applies
to a realm still holding counties — never looks at it. Traced turn by turn, the lowest health
any surviving AI realm reaches in the forty turns is 45 (realm 2, turn 29) and it recovers.
The green is not the raid going quiet: blinding the raid finder still moves that world (realm
2 ends on 4 counties at health 67 rather than 3 at 85; realm 5 keeps 10 grain fields rather
than 6), so the raids bite and the fix is live. On the England fixture the finder changes
nothing in forty turns — blinded and live give identical scoreboards.

So the assertion was never weakened and never had to be: it is not a rule — a realm raided
off its harvest starves, and `Ai_FindStandingCropTile` exists so that it can — but it is also
not in danger on this trajectory. What was wrong is that a long run was the **only** witness
of the rule, which is C184's finding in another costume: a claim you can only observe as a
shifted trajectory is a claim nobody can ablate. So it is dealt deliberately instead.
`crates/l2-kingdom/tests/ai_raid.rs` sows a county, ticks it through the four seasons and asks
`aim_tile` what the raid finder sees, then tramples one field and asks again. Three ablations,
each one line, each red: C124's crop word in Spring, Summer and Autumn; `Aim::StandingCrop`'s
`terrain > 2`; and the sowing's write of `+0x206`.

---

**C196 — a gated invention, a silence that is the original's, and two
copies of one sentence. Three small holes, and each was hidden by something that
had been written down truthfully.**

Three reports, three different ways of not being findable.

**1. A besieging army had no mark, and the marker we did draw was in the way of finding
the one that exists.** `FUN_00407F82` (`0x00407F82`) blits `Flags1a.pl8` frame `0x82` —
24 × 28, and the **last** frame in a sheet of 131 — at `(+8, −0x38)` from the *castle*
tile's origin, then prints the besieger's `+0x19C`, siege seasons left, ten pixels lower.
It is called from `Sprite_TopIt`'s castle arm, which reaches the besieger through the
garrison: `units[county.garrisonUnit].besiegedBy`.

Ours drew a red dot over the **army**. It carried an honest comment saying so — *"ours is a
dot over the unit, where the original draws nothing, so it is debug overlay only and
`FUN_00407F82` stays a missing draw"* — and C173 had gated it on that reading. **The gate
is what kept it.** A visible invention gets reported by a player; a gated one is correct by
construction and stops being looked at, and the missing draw was named two lines above the
code that stood in for it. `docs/draws-map.md` §7 now says this where the dot used to be
counted.

Four details of the draw are decisions rather than transcription, and three of them would
have been got wrong by reading the name:

* **`Ui_DrawNumberRight` centres.** `docs/symbols.md` `0x004030C6` has carried that
  correction since C119/C140 and the name is still the false one, deliberately. The count
  is centred in **frame `0x82`'s own width** — `g_spriteWidth = *(short *)(g_flagsSheet +
  0x828)`, which is `8 + 0x82 * 0x10`, the width field of that frame's record.
* **The suffix is one space and it is inside the measure.** `DAT_004D2094` is `20 00 00 00`
  at file offset `0xD0294`.
* **The count is flat.** `DAT_005AEA40 = 1` brackets the `Ui_DrawNumberRight` and is cleared
  after it, so it is not the map's ordinary shadowed body text.
* **`if (0x18 < y)` is the count's only clip.** The blit is clipped by `Clip_Horizontal` /
  `Clip_Vertical`; the number is not, and that test is all that keeps it off the menu bar.

**And the arm is not reachable at the far zoom**, which is the original's own defect and is
reproduced: `Sprite_TopIt` computes `(2, −0x28)` for `g_mapZoom == 2` and the whole body of
`FUN_00407F82` is inside `if (g_mapZoom == 0)`. `docs/bugs.md` `D41`,
verified from the shipped bytes rather than from the decompiler's C.

**The audit's own note about this arm was wrong in a way worth keeping.**
`docs/draws-map.md` §6 listed it as *"needs a live siege on the campaign map, which the
battle triple does not carry"* — read as *wait for a fixture*. What the arm wants is **two
bytes**, `+0x19A` on the garrison and `+0x19C` on the besieger, which is the same shape as
the mercenary marker's one byte and was staged in a test in four lines. A list of things
*"not observed"* that does not distinguish *needs a save we do not have* from *needs a
field written by hand* will keep the second kind unobserved indefinitely.

**2. Picking a merchant says nothing, and that is the original.** Nothing is built, and the
finding is the point — `CLAUDE.md` rule 5's *"we could not find it" is a finding to report*.
`FUN_004B37BC` (`0x004B37BC`) is the last statement of **both** functions that open screen
`0x04`, and its ladder is kinds 1, 4 and 2 plus the castle branch; kind 3, the merchant,
falls out of the bottom and returns.

**The silence was already built, already asserted and already ablated**, in
`tests/audio_wiring.rs` `the_information_panel_speaks_the_unit_it_opened_on` — *"that hole
is what this asserts hardest, because the easy mistake is to fill it"*. What was open was
the *question*, carried by a ledger row whose next step read *"find whether the original
speaks for a merchant at all"*. Same shape as the other two: a green test that answers a
question, and a written-down question that does not know about it.

Three readings, and the third is the one that closes it rather than merely failing to open
it:

* **the ladder** has no arm;
* **four `S031_*.wav` ship** and there is no fifth;
* **`L2.eng` group 31 holds five unit descriptions and four of them have voices.** Indices
  13 … 16 are *"These starving revolutionaries…"*, *"This transport is moving goods…"*,
  *"This is an enemy army."* and *"This is one of your armies."* — the four, in exactly the
  order `S031_01` … `04`. Index **12** is *"Merchants allow a county to buy needed supplies
  and raise revenue by selling goods."*: the one member of the run with prose and no
  recording.

That third reading is rule 6 used as an *absence* test. A group with one consumer is that
screen's vocabulary, so a word in the group with no voice beside it is evidence about the
recording session and not about our search. Without it the answer would have been *"we
looked and did not find one"*, which is a statement about the looking.

`Map_Click` (`0x0043CE1A`) corroborates it from the third side: a left click on your own
merchant sets `g_screenId = 8`, the stall, and on somebody else's enqueues group `0x70`. The
information panel never opens on a merchant from the left button at all. `docs/audio.json`
`FUN_004b37bc#1` carries the whole of this as its note; nothing about the inventory's
numbers moves, and `sounds.js --check` still agrees with the corpus on all 143 sites.

**3. Two screens still invented a lord's name, and C189 is why they were easy to miss.**
C189 fixed the court and the battle prompt and named the rule — `g_playerNames`, then
`L2.eng` group 7 indexed by the realm's **lord**, which is the pair `Game_NewGame` itself
seeds the array from (`Eng_Seek(7, realm[+0x07])`, then sixteen bytes copied). It did not
sweep for the other copies. There were three, in two files, and each had grown its own
`REALM n`:

| where | the original's draw |
|---|---|
| `county.rs`, the strip's third line | `CountyStrip_Draw` `0x0040F7D3` — `FUN_004025D7(&g_playerNames + owner * 0x2C, 0x1E0, 0x118, 0xA0, …)` |
| `diplomacy.rs`, the screen and its cards | `Diplo_DrawScreen` `0x00416CF3` — `Ui_DrawText(… + g_diploTarget * 0x2C, 0xD0, 0x3D, heading)`; `Diplo_DrawLordCard` `0x004171EE` — `Ui_DrawText(… + realm * 0x2C, 0x20, slot * 100 + 0x83, body)` |
| `diplomacy.rs`, the three compose dialogs | `Diplo_DrawGiftGold` `0x00417960`, `Diplo_DrawLetter` `0x00417AEF`, `Diplo_DrawCountyRequest` `0x00417CEF` — all `Ui_DrawText(… + g_diploTarget * 0x2C, g_penAdvance + …, body)` |

All five are one draw in the original and are now one function, `screens::message::lord_name`.
**Nothing changes for a game started through the front end** — the array is read first at
every site, which is what the compose screen's own comment was protecting — and on a `.sav`,
which carries no typed names into this tree, the county strip, the diplomacy screen, its
lord cards and its three compose dialogs stop saying `REALM n` and start saying what group 7
calls that realm's lord.

**Two of the three copies were each documented as correct**, with the other cited as
precedent: the compose screen's comment said *"`screens/county.rs` settled the same question
the same way"*, and `county.rs` explained at length why its fallback was *"not decoration"*.
Both sentences were true. Neither was the whole rule, and each made the other look reviewed.

**What was left alone, and it is the real fix.** `g_saveBlocks` entry 2 is
`{0x00553D50, 264}` — 6 × 0x2C, the player-name table — so **the names are in the `.sav`**
and `l2_formats::save` does not read them. Every fallback above exists because of that one
gap. It is not touched here: `crates/l2-formats/` is the lead's (`docs/agents.md`), and the
four-byte discrepancy between the block's base and `g_playerNames` at `0x00553D54` wants
settling against a real file before anything reads it.

---

**C197 — the turn was a screen's job and it is the frame's,
and the `else` branch nobody built gave away a county the original refuses.**

Two defects the capture-letters branch found and left, and they are one entry because both
were read out of the same two functions.

### The turn stopped under an open letter, because we gave it to a screen

`Battle_Frame` (`0x004B99C0`) ends its inner loop with

```c
if ((g_battlePhase == 0) && (ticksDue != 0)) { FUN_0040490d(); Turn_Tick(); Units_Tick(); }
else if ((g_battlePhase == 2) && (ticksDue != 0)) { …the battle's passes… }
```

and **there is no `g_screenId` test on either arm** — `[V]`, read whole. Everything in the
loop's tail that does dispatch on the screen (`Screen_Draw`, `Screen_FrameInput`, the
cursor ladder) is drawing and input. So in the original the campaign winds on under a
county panel, a village, an open menu and the message scroll alike, and the only thing
that suspends it is a battle.

Ours wound the turn out of `MapScreen::update`, and `Machine::update` gives `update` to
the **top** screen only. The scroll is not a screen in the original at all — `g_screenId`
does not change when a letter opens, which is exactly why `Msg_Pump`'s own ladder goes on
treating the map as the screen — but it is one here, so the frame a letter arrived was the
frame the campaign stopped on, for good: single player clamps the message timer, so the
window waits for a click that the test never gives and a player might not either.

**The absence is the finding, and an absence is not visible from the half of the function
you already read.** The pump ladder, the turn clock and the tool tips were all lifted into
the frame driver *because they had no screen*; the turn was left where it was because the
map plainly owns it. It does not: `Screen::update` is `Screen_FrameInput`'s half of a
frame and the new `Screen::wind_turn` is the loop's, and the campaign map is the one screen
that has both. `Machine::wind_turn` calls it at whatever depth the map sits.

Two things it does **not** change. A battle still suspends the turn, which is
`g_battlePhase != 0` and here is `Game::battle` plus the battlefield screen. And a turn
waiting on screen `0x12` or `0x13` still waits: `resume_turn` asks for the prompt before it
ticks anything, so the phase machine is its own gate — the driver only refuses to put up a
screen already on the stack, which it never had to before because it never ran there.

### A county too far to govern goes independent, and ours handed it over

`County_ChangeOwner` (`0x004A72FE`) is one `if` with two branches and we had built one:

```c
if (realm[new].countyCount == 0 || County_BordersRealm(new, county)) { …the capture… }
else {
    if (newOwner == g_localPlayer) Msg_Enqueue(0, g_localPlayer, 0x81, 0, 0, county, 0, 0);
    County_MakeIndependent(county);
}
```

The `else` was marked NOT PORTED and `Capture::governable` carried the test out so the
letter layer could stay silent rather than post a letter the world contradicted. It is
built now, and read whole rather than summarised: the branch does **not** increment
`countyCount`, does not `Realm_RecountStrength` the loser, does not take the happiness
penalty, does not write the shield and does not raise the peak. Its one letter is inside
`if (newOwner == g_localPlayer)`, so **the loser is never told** — he finds out from the
map.

`County_MakeIndependent` (`0x004AC3C6`) already existed as a `Kingdom` method for
secession, revolt and elimination; it is now `conquest::make_independent` and the kingdom
delegates, because the fourth caller is inside `change_owner` where neither the tables nor
`g_seasonNext`, `g_optAdvancedFarming`, `g_optArmiesEat` and `g_countyCount` reach.
`conquest::Restore` carries those four and `Kingdom::restore()` builds it.

**What moved in the lockstep digest.** Only on an ungovernable capture, and there: section
`counties` — `owner` is 0 rather than the taker, all four `industry[].enabled` and
`castle_switch` are off, `garrison_unit` is cleared, `happiness` and `shown_events` are
*unchanged* because the penalty is in the other branch, and `Labour_Allocate`,
`Ration_Apply`, `County_RefreshEstimates` and `Tax_RecomputePreview` move the `job`,
`grain`, `herd`, `ration`, `food`, `field` and `tax` sub-sections; section `realms` —
`county_count` does not rise and neither does `peak_counties`. A governable capture is
byte-identical to before. The differential's four pairs did not move (932/913, 279/270)
and `long_game`'s hundred- and four-hundred-turn sweeps are unchanged. The frame-driver
half adds no state at all: it changes *which frame* a tick lands on, and it stops that
being a function of which screen a peer has open.

**And a trap the fix laid bare.** `County_BordersRealm` reads `county.neighbours`, so a
test world without an adjacency list now sends every capture down the `else` branch: six
tests in `conquest.rs` and one in `tests/campaign.rs` went red saying *"the county did not
change hands"* when what they were missing was two lines of neighbour data.
`tests/military.rs` had already recorded the same trap from the secession side. The
adjacency is in `world()` and in `campaign.rs`'s `kingdom()` now, with the reason beside
it, and county 3 is deliberately left with none — it is what the `else` branch is tested
with.

---

**C198 — the lords' names were in the `.sav` all along, and the four
bytes in front of them are a DirectPlay id.**

> *"a loaded game forgets what the lords are called."*

`g_saveBlocks` entry 2 is `{0x00553D50, 264}` and `264 = 6 × 0x2C`, so the names are saved.
`l2_formats::save` did not read them: `Game::player_names` came back empty from every load,
and every `"REALM n"` a player met was that. **The person's own name was the worst of it.**
`screens::message::lord_name` falls back to `L2.eng` group 7 indexed by the realm's lord, and
`Realms_AssignLords` (`0x0049CAAA`) writes `lord = 0` for a realm somebody is *playing* —
group 7 index 0 is **`"No player"`**, which is what the court page and the diplomacy heading
called the reader of them.

**The four bytes had to be settled first, and they were the whole shape of the record.**
`0x00553D50` is four lower than `g_playerNames` (`0x00553D54`), which reads as a block
misaligned by a dword with realm 5's record hanging four bytes past its end. It is not: the
block's base is the base of a **six-slot player table**, and the name is the slot's `+0x04`.
`Player_SetHuman` (`0x0049BAE9`) writes both halves from one argument list at one stride —
`FUN_00401136(name, &g_playerNames + realm * 0x2c, 0x1f)` and
`*(int *)(&DAT_00553d50 + realm * 0x2c) = dpId` — and Ghidra's own view agrees: `0x00553D50`
has **120 immediate references in the image and `0x00553D54` has none**, because every access
to the record goes through the table base. So the block covers slots 0 … 5 exactly and
nothing is truncated. **[V]**

`+0x00` is the **DirectPlay player id** of the person driving the realm, 0 for nobody:
`FUN_0043E9E2` clears realms 1 … 5 and writes `g_dpPlayerId` into the local player's;
`FUN_0043E98B(id)` maps an id back to a realm; `Net_ReadField(&DAT_00553d50 + realm * 0x2c, 4)`
puts it on the wire; `Player_SetHuman` writes it unless the caller passes the **999** sentinel;
and `Mp_DropDepartedPlayers` (`0x0049B2A3`) eliminates a realm marked human whose copy *has
gone to zero*. **That promotes a hypothesis rather than adding a name.** `g_playerSlots` was
already in `docs/hypotheses.json` at `role` confidence with the caveat *"what the table
actually holds — a network peer, a save slot, a controller — is not established"*. It is a
network peer. The entry is moved to `docs/symbols.json` as verified.

**Measured over eighteen saves** — eleven fixtures and every `.sav` in the two installs.
`+0x00` is zero in all of them, which is a single-player game having no connection, and it is
the assertion that will tell us when we finally have a multiplayer save. Two more fields agree
at the same stride and are the reason the base is not four bytes out: the local player's
`+0x25` equals `g_realms[p].shieldIndex` in every save (two fields written by different code,
at two different strides, 1 in six of them and 5 in the other twelve), and every realm in play
has a terminated name. **Ablated**: `PLAYER_BASE` moved up to `0x00553D54` and the id reads
`0x79616C50` — `"Play"`.

The names load in `l2_game::scenario::from_save`, raw and beside `realm_colour`, and the
fallback stays where it is. **Nothing moves in the lockstep digest**: the digest is
`Canonical::hash_of(kingdom)` and `player_names` is on `Game`, which is the reason it was put
there. `l2_game::save` already carried it, so our own `.l2sav` needed no version bump.

**Left alone, and named rather than fixed.** `screens/ratings.rs` still draws
`format!("PLAYER {}", b + 1)` where `Screen_BattleMasterRatings` (`0x00421707`) draws
`g_playerNames[g_localPlayer]` and `g_playerNames[DAT_0056D5CC]`: the block index it has is
not a realm id, so wiring it is a change to `Ratings`, not a call swap.
`screens/county.rs`'s `CountyStrip_Draw` arm keeps its own copy of the fallback — it now shows
the real name like everything else, but it drops straight to `"REALM n"` where `lord_name`
would try group 7 first.

**And a `[V]` that was four bytes wrong, in the place `CLAUDE.md` warns about.**
`l2_scenario::newgame` and `docs/rules.md` both said the human's shield is at
`g_playerNames + realm * 0x2C + 0x25`. `Realms_AssignLords` reads `(&DAT_00553d75)[realm * 0x2c]`,
which is the *table's* `+0x25` and the name's `+0x21`. Nothing was built on the wrong one —
the colour is passed into `NewGame` rather than read out of a save — so this cost nothing, and
it is exactly the sentence that would have cost the next person a day. `crates/l2-game/src/text.rs`
had the record right the whole time and no reader used it.

---

**C199 — the tile panel's castle arm, four
figures a comment said we did not carry, and the selection the right button
also makes. Three gaps in one screen, and two of them were held open by a
sentence that had been true.**

**1. `TileInfo_DrawCastle` (`0x0041DA2F`) was not built.** Right-clicking a castle gave a
box with a heading in it. The arm is two halves that share almost nothing:

* **intact** (`castleDegraded == 0`, `+0x1C2` clear) — 71/16 and the tax bonus, 71/11 and
  the barracks cap with 71/12 after it, and, with a garrison, the men and 71/13 or 71/19
  for somebody else's, then 71/14 and the widget;
* **degraded** — the builders at `(0x68, R + 0x88)` and then
  `Castle_DrawStatusBlock(county, 8, 0x30, R)`, the same block `FUN_00414220` draws on the
  job page at `(-0x20, 0x40, 0)`. So the county *building* a castle is the one that shows
  the stone and wood owed and the seasons left, and the finished castle never does.

**A ruined county (`+0x1C2`) draws nothing at all** — the heading stands over an empty
box. That is the original's and is reproduced; `FUN_0041BEFE` gives it row `0x11` rather
than the `0x0E` an intact castle gets, so the box is short and the emptiness is smaller
than it would otherwise look.

`FUN_0041BEFE`'s `0x80` arm is the layout: `0x0A` for a degraded castle of your own,
`0x0E` for an intact unruined one, `0x11` for everything else including every resource
site. **`0x0A` is the tallest tile layout in the game** and `Layout::ALL` already carried
it, with the condition, and nothing produced it — the ladder had been read and only the
farmland and county-town arms built from it. A row in a table of eleven that no code path
can reach is not a gap anybody trips over.

The tax and barracks words are `Castle_DrawStatusBlock`'s own two table reads, one word
low, so `docs/bugs.md`'s *"Barracks for 2500 troops."* on a county with no castle is
reproduced on this screen too. It is one function, `screens::job::castle_word`, and both
screens call it.

**2. Four figures were drawn by the job page and not here, because a comment said they
were not carried.** `screens/info.rs`'s module doc read *"Two figures are not carried and
are not drawn … `docs/stored-fields.json` has all four as excluded"*, and the two report
painters each carried a `NOT PORTED` beside the line. **All four are `imported`** —
`grain_event_change` (`+0x278`), `herd_event_change` (`+0x274`),
`grain_weather_change` (`+0x24C`) and `herd_weather_change` (`+0x270`) — and
`Panel_JobGrain` and `Panel_JobCattle` have been drawing them since C164.

The comment was true when it was written. What made it expensive is that it was
**specific**: it named the four offsets, cited the file that would have contradicted it,
and explained why the *"no outside factors"* sentence was exact. A vague note gets
re-checked; a well-sourced one gets believed, which is `CLAUDE.md`'s warning about `[V]` in
`docs/formats/` arriving at a different document. The half that stayed right through all of
it is the conditional sentence: the painter draws 77/24 only when the figure is zero, and
ours drew it on the event id instead, which agreed on every save because the figure is zero
in all of them.

**3. `FUN_0043CAF4` also selects a county, and the guard nobody would guess is the third
one.** Between `Map_ResolvePick` and the panel:

```c
if ((g_pickedTileCounty != 0) && (g_pickedTileCounty != g_selectedCounty) &&
    (g_pickedTileUnit == 0)) {
  DAT_0053f0dc = g_counties[g_pickedTileCounty].townTile;
  if (DAT_0053f0dc != 0) { g_selectedCounty = …; Map_CentreOnTile(DAT_0053f0dc); }
}
```

Right-clicking bare ground of another county selects it and recentres on its **town**;
right-clicking a **unit** standing on that same ground selects nothing, because the panel
that comes up is about the army. A county with no town square shows its panel and leaves
the sidebar alone — the same shape `Map_Click`'s industry arm has, without that arm's
outright refusal.

**No new input arm.** This is a second effect of a gesture `docs/arms.json` already has
(`0x0042FF10/map-right-opens-info`), which is why the inventory could not have found it:
the arms count says *which* controls a screen answers, and a handler that does two things
is one arm either way. `CLAUDE.md` rule 5's two measurements do not cover a third thing,
and this is it.

---

**C200 — a man may not change his mind once he has stepped, and the siege
picks the other palette. Both were in one `if`, and one of them had been read
backwards for the whole life of the document.**

Two reports on one screen: men still *"reset on their square"* occasionally after C183, and
a siege runs in the field battle's colours.

**The crossing.** `BattleMan_Step` (`0x0048F1DD`) decompiled end to end settles what §7 and
`symbols.json` had both recorded the other way round. Bit 0 of `stepFlags` (`+0x34`) gates
the function: while it is clear the figure is mid-crossing and the body returns after
counting `moveTick` and `walking`, **above** `Melee_AdjacentEnemyDir`, above
`Dir_FromDelta` / `BattleMan_NextPathDir` and above `BattleMan_TryStepDir`. A step
`Cell_TryEnter` accepts runs `stepFlags &= ~1; dirc = dir; walking = 1;
FUN_00491b1f(man)` — and `FUN_00491b1f` is the move. So the commit is the **first** tick of
a crossing, not the last; `dirc` is fixed for it; and nothing can refuse the step half-way.

`BattleMan_StateMelee` (`0x004831D8`) is the same rule from the other side and is what the
last of the jumps needed: `Anim_Strike(); … Melee_Tick(); if ((stepFlags & 1) == 0 &&
BattleMan_Step(1)) Anim_Walk();`. A man engaged part-way across a cell **finishes the
crossing**, walking, and `noInterrupt = 1` makes his landing tick decide nothing. Ours
zeroed his progress and struck where he stood, which teleported him up to 30 pixels onto
the cell he was still entering. `dirc` is not rewritten there either — the 999 arm writes
`dirc2` (`+0x19`, the strike frame) and leaves `dirc` (`+0x18`, the sub-cell offset) alone.

Measured, one 42-figure battle, 1,200 ticks, `no_drawn_man_ever_jumps_half_a_cell_in_one_tick`:
drawn jumps of 16 px or more **324 → 0**. (C183's 1,317 → 504 was the same defect on a
different scenario; this branch's baseline on its own scenario is the 324.) The 324 were
mid-crossing turns and refusals after the walk; the last 15 before the melee clause were
figures engaged mid-crossing. **No `BattleMen_SwapPlaces` fires in that battle** — a swap
*is* a teleport in the original, and one would have shown as a 32-pixel jump.

**What moves in the lockstep digest.** `Progress` gains `free`, `stepFlags` bit 0, and it
is hashed: a figure that has just landed and one that has just been ordered both read
`substep == 0`, so `substep` does not cover it, and two peers that disagreed would have one
man finishing a crossing while the other re-chose his direction. **The census could not
have caught that** — it walked `Fighter`, `Missile` and `SiegeState`, and `progress` is one
field name there and three in the digest. `Progress` is in the walk now; C39's rule needed
applying one level down.

**And a speed correction nobody was looking for.** `walking` runs 1, 3 … 15 — **eight**
sub-steps — so a cell costs `8 * (moveDelay + 1)` ticks and not `9 * (…)`: 8 for a knight,
40 for a pikeman, 48 for an engine. §7's `9` came from reading the counter as starting at
zero, which it never does. Every relative speed the manual states is a ratio and is
unchanged, which is exactly why the error survived: `speeds_match_what_the_manual_says` was
green against both numbers. A test named
`nobody_moves_before_their_troops_move_delay_has_elapsed` asserted our order rather than the
original's and is now `a_committed_man_is_on_the_next_cell_at_once_and_holds_it_for_the_delay`.

**The palette, and the measurement is the point.** `Screen_DrawBattlefield` (`0x004233F7`)
ends every repaint with `if (g_battleIsSiege == 0) Palette_Set(0x568ee0); else
Palette_Set(0x5675a0);` — records 2 and 1 of `g_preloadTable` (`0x004D9F48`),
`t32_bat1.256` and `t32_stn1.256`, spelled in the table's own bytes and loaded by
`Res_LoadStatic`'s (`0x00499859`) index ladder. C183 registered only the first and wrote
the siege arm off as *"belongs with `t32_stn1.pl8`; one without the other would be wrong
both ways"*. It is registered now and `BattlefieldScreen` latches `g_battleIsSiege` in
`update` for `palette`, which is handed no context.

**It changes no pixel today.** The two files differ on **3 of 256 entries** — 0, 115 and
172; 172 is field green `(33, 49, 18)` against siege brown `(37, 13, 2)` — and **none of
those indices appears anywhere in a painted siege**, because we draw a siege from
`T32_bat1.pl8` and the three belong to `T32_stn1.pl8`, which is not ported: 898 of that
sheet's 215,079 opaque pixels would move. So the arm is faithful, cheap and worth nothing
on screen until the siege tileset lands. Recorded because the report described it as a
visible fault and it is not one; the counts are printed by
`a_siege_is_shown_in_t32_stn1_and_that_changes_the_picture` on every run rather than
asserted, since both come from the player's own files.

**Left alone.** `FUN_004904EC`, the mover's side-step, and `Path_DetourTooLong`'s
give-up — both still unreproduced, both still recorded in §8.3a rather than built. The
siege tileset. And `BattleMen_SwapPlaces`' teleport, which is the original's.
