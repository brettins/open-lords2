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
made the first claim, from a search too narrow, and was wrong.

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

**CNEW-withdrawal — "Nobody dies" was three-quarters false, and the quarter that was true was
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

**C71 — "Every casualty in every battle is unkilled" was wrong, and what it hid was better than
what it claimed: a function in no document that halves a retreating army *before* the rule that
destroys it.**

**The premise, and it came from us.** A brief went out saying that retreat and autocalc discard
casualties and that *"every casualty we have fought so far is unkilled."* It was repeated from an
agent's report, restated by the coordinator, and acted on.

**Measured, it is false and always was.** The forty-turn England run fights **five battles and
kills 1,951 men**. Every one of them is an autocalc, because an AI-versus-AI battle never enters
the simulation at all — and writing survivors into the campaign records is the autocalc's entire
purpose. The before/after table is identical on every row.

`Battle_WriteBackCasualties` has **five call sites**, we implement it, and ablating our
`write_back` turns **three** tests red: `taking_the_field_runs_the_real_simulation_and_hands_the_result_back`,
`the_same_position_can_be_fought_for_real_and_still_comes_back`, and
`the_two_thumbs_reach_the_two_ways_a_battle_can_be_settled`. (Counted with `--no-fail-fast`; the
first run stopped after one crate and reported a single failure, which is its own small lesson
about reading an ablation.)

The original claim was true of **exactly one path** — the solo retreat arm — and was stated as
though it were the seam. **A true statement about one branch, promoted to a statement about the
subsystem**, is the third instance this week of a correct observation producing a wrong
inference, and the first where the promotion happened in a *brief* rather than in a tool's
output. `docs/agents.md` carries all three together, because the pattern is now clearly not
confined to tools.

**What was actually missing.** `Army_WithdrawCasualties` (`0x004AD8CC`) — in no document, no
`symbols.json` entry, and no line of our code. It halves every troop line, **wipes any line under
11**, leaves mercenaries untouched, and — this is the part that matters — it runs **above** the
loser branch, so the `menTotal < 50` test reads the **halved** total.

We tested the unhalved one. **An 80-man army survived where the original halves it to 40 and
destroys it.**

**And the shipped `Readme.txt` says so, in English, and we have quoted that sentence twice.**
*"less than 50 men **after** retreating."* `CLAUDE.md` promotes `Readme.txt` to a first-class
oracle precisely because it post-dates the manual and corrects it; this project cited it twice
while implementing the opposite order of operations. **Citing an oracle is not the same as reading
it**, and that is a short lesson with its own entry in `docs/agents.md`.

**It was unreachable, and that is C27's eighth instance.** `g_battleWithdrawal` has exactly one
writer in the binary, `UnitOrder_SiegeAttKnight`, and we had every other line of that function and
not the three that raise the flag. So `End::Withdrawal` was reachable only from two unit tests,
and the **entire withdrawal half of `Battle_ReturnToCampaign` was dead code** — written, correct,
and impossible to enter.

This is the first of the eight found by **adding a writer** rather than by chasing a reader, and
the difference is worth keeping. The other seven were found by asking *"who writes this field?"*
and getting no answer. Here the field had a writer in the original that we had simply not
transcribed, so the question *"is this reachable?"* had a comfortable answer — *yes, from the
tests* — and the missing piece was three lines inside a function we thought we had finished.
Adding it also removed a stall: a fought siege that previously ran to `MAX_TICKS` and came back
`Stalled { ticks: 12000 }`.

**A call graph that did not exist.** `victory.rs`'s doc comment has listed the two post-battle
sites among `Realm_RecountStrength`'s callers since the day it was written, **and nothing called
it.** Not a stale comment — a comment that was never true, describing a call graph as though it
were the code beneath it. It is called on the loser's realm now.

**And a genuine defect of the original's, catalogued rather than fixed.** The retreat/autocalc
split is on `g_multiplayer`: the network arm writes casualties back first and the solo arm does
not. Same button, two meanings. Worse, because the autocalc clears the withdrawal flag before it
runs, **pressing Retreat does not retreat** — you auto-resolve, and losing destroys your army.
That is behaviour, it is in `docs/bugs.md`, and it is not ours to correct.

**Three things left open and declared rather than quietly handled**, which is why this entry
trusts the rest: mercenary figures lose their band on a fought battle because `l2_sim::Figure`
carries no flag for it; the loser-survives branch clears a link the original leaves
half-connected, marked deliberate by an earlier decision; and `engagement::resolve*` called
directly does not recount the losing realm, though all four roads a player can take go through
the path that does.

## Open questions

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
