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
  The one piece still inferred is the castle ceiling's materials gate —
  `crates/l2-kingdom/tests/labour_gap.rs` and `docs/kingdom.md` §14.4.
- **The castle's materials are debited up front, and the original delivers them over time.**
  `Castle_BuildEstimate` returns a labour ceiling of **0** until six words at county
  `+0x1CC … +0x1E0` say the wood and stone have arrived; `industry::order_castle` takes the
  whole cost the moment the castle is ordered, so this tree's gate is permanently open. The
  arithmetic given a complete delivery is reproduced and the delivery is not modelled. It is
  the last unreproduced call of `County_RefreshEstimates`.
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
