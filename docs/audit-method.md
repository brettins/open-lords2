# Method audit — are we using the right techniques?

Audited 2026-09-07 against the working tree at commit `d09de29`. This document asks one
question: **is this project using the right techniques, and are its stated obstacles
real?** It is not a roadmap and it proposes no features.

Nothing outside this file and `tools/audit/` was touched. Nothing was staged or committed.
No process was launched. The game was never started — §6 explains why, and says what would
have justified it.

New scripts written for this audit, all static and all fast:

| script | what it does | measured cost |
|---|---|---|
| `tools/audit/rva.js` | read any virtual address out of `Lords2.exe` — hex dump or typed array — with no Ghidra | **44 ms** |
| `tools/audit/coverage.js` | cross-reference every function address the docs name against `tools/oracle/decomp/index.txt` | **55 ms** |
| `tools/audit/tablediff.js` | diff every crate constant table against the binary, reading the address out of the crate's own doc comment | **81 ms** |

---

## The three that matter

### F1 — `Path_Search` never clears 36% of its visit counters, and we do. The reason given for not knowing was that the answer was elsewhere; it was already on disk.

**The claim.** `docs/decisions.md`, Open questions:

> Whether `Path_Search`'s visit counters are fully cleared between searches. The clear is
> `FUN_004b3e51(&g_pathVisitCount, 0x1000)` against a 6,400-cell grid; if that count is
> bytes rather than dwords, the last 2,304 cells keep stale counts from the previous search
> and deferral behaves differently on the bottom third of the map. **Not resolvable from the
> decompilation alone — it needs the callee's signature.**

**What I measured.** The callee's *body* is in the decompilation, at
`tools/oracle/decomp/004b0000.c:2508`. It is a byte-count `memset`: sixteen dword stores
followed by `param_2 = param_2 + -0x40`, then a single-byte tail loop. I did not trust the
decompiler on a question this load-bearing, so I read the instruction stream
(`node tools/audit/rva.js 4b3e51 111`):

```
004b3e51  ... 8b 7d 08        mov edi,[ebp+8]      ; dst
              8b 4d 0c        mov ecx,[ebp+0xc]    ; count
              83 f9 40        cmp ecx,0x40
   16 x       89 47 xx        mov [edi+0x00..0x3c],eax   ; 16 dwords = 64 bytes
              83 c7 40        add edi,0x40
              83 e9 40        sub ecx,0x40         ; <-- count is in BYTES
   tail:      88 07           mov [edi],al         ; one byte
              83 e9 01        sub ecx,1
```

And the call site (`node tools/audit/rva.js 4709d6 32`):

```
004709de  68 00 10 00 00     push 0x1000        ; 4096 BYTES
004709e3  68 70 64 4f 00     push 0x004F6470    ; g_pathVisitCount
004709e8  e8 64 34 04 00     call 0x004B3E51
```

Three independent corroborations that the argument is a byte count, not an element count:

1. The sibling call in the same function passes `0x3200` for `g_pathCost`, which
   `docs/symbols.md:318` documents as **u16 × 6400 = 12,800 = 0x3200 bytes**.
2. `docs/battle.md:715` already records `g_pathVisitCount` as **`u8` per cell**.
3. `g_pathVisitCount` is at `0x004F6470`; `0x004F6470 + 6400 = 0x004F7D70`, which is exactly
   `DAT_004f7d70`, the next global `Path_Search` itself uses. The array is 6,400 bytes and
   ends where the next variable begins.

**So: 4,096 of 6,400 counters are cleared. Cells 4096…6399 — 2,304 cells, rows 51 through
79 of the 80×80 grid — carry their counts into the next search.** The question is settled,
and the answer is the direction the open question feared.

**Where it reaches us.** `crates/l2-sim/src/pathfind.rs:201` is
`let mut visits = vec![0u8; CELLS];` — a fresh, fully-zeroed 6,400-entry array every search.
The rest of that port is careful and correct: I checked the 0x1900 queue wrap, the
non-relaxing cost write, the `stepCost == 0` short circuit and the off-by-one on the visit
comparison, and all four match. This one does not. Where `step_cost != 0` in the bottom 36%
of a battlefield, a stale counter at or above the cell's step cost makes that cell expand on
its **first** pop — terrain cost silently not charged.

Scope, honestly: `docs/battle.md` §8.3 records that on a `.skr` battlefield every step costs
zero, so on skirmish maps the stale counters are inert. They are live on the random and
castle battlefields built by `Battlefield_BuildRandom` / `Battlefield_BuildCastle`.

**What should change.** Decide explicitly whether to reproduce the original's partial clear
(bug-for-bug) or to diverge on purpose, and say which in the module doc — this is precisely
the class of thing C12 was about. And retire the habit the claim exemplifies: *"not
resolvable — it needs X"* where X is an artefact already sitting in the repo.

---

### F2 — Three documents cite "not decompiled" and "the Ghidra lock" as live blockers. The functions are on disk. One document contradicts itself about it.

**The claims.**

* `docs/battle-ai.md:756` — *"**`Battlefield_BuildCastle` was not decompiled**, so the castle
  position tables … are named by *use*, not by content. **Every siege claim in §6 rests on
  that.**"*
* `docs/battle-ai.md:781` — *"`Path_SearchSiege` … was read only far enough to confirm it is
  the same shape. Its differences were not enumerated."*
* `docs/battle.md:199` — the elevation byte's origin is unknown because
  `Battlefield_BuildRandom` / `Battlefield_BuildCastle` *"neither was traced"*.
* `docs/audit.md` §8 — every Ghidra-derived claim in the knowledge base *"was taken on
  trust"*, because *"Running Ghidra would have contended for the project lock with agents
  working now."*
* `docs/kingdom.md:1070` lists *"The fourteen AI turn handlers"* as the largest remaining
  piece — while `docs/kingdom.md:240` in the same file says *"All fourteen handlers have now
  been decompiled"*.

**What I measured** (`node tools/audit/coverage.js check …`, 55 ms):

```
PRESENT  Battlefield_BuildCastle   0x0047c4ba   2567 bytes  -> tools/oracle/decomp/00470000.c
PRESENT  Battlefield_BuildRandom   0x0047aaa3   3599 bytes  -> tools/oracle/decomp/00470000.c
PRESENT  Path_SearchSiege          0x00471718   1287 bytes  -> tools/oracle/decomp/00470000.c
```

The tree holds **2,452 functions, 3.3 MB**, and it is current: `decomp/index.txt` is stamped
21:14 against `docs/symbols.json` at 18:10, and all 230 function symbols in `symbols.json`
are correctly applied in it. Across the whole `docs/` tree the project names **569 distinct
addresses**: 292 are function entry points and **all 292 are in `decomp/`**, 164 are known
data globals, and 113 are mid-function addresses, struct offsets or unlocated. There is no
function the documents name that the tree does not hold.

The Ghidra-lock argument is likewise spent. Re-verifying a decompiler-derived claim now costs
a `rg` over a directory. It contends with nothing.

**This is C16's error at document scale.** C16 was one person concluding "only a running
process can supply this" when the value was in `.text`. Here the same shape is repeated:
*"we could not read that function"* was true when it was written and is a statement about a
tooling era that ended at commit `9f97838` ("Decompile the whole binary once, instead of per
question"). Nothing swept the documents afterwards, so a solved cost is still being paid
forward as an unknown — and in `battle-ai.md`'s case it is explicitly load-bearing: *every
siege claim in §6* is disclaimed against a file anyone can open.

**What should change.** A sweep of the three "What is still unknown" sections
(`battle.md` §11/§13.9, `battle-ai.md` §9, `kingdom.md` §12, `audit.md` §8), replacing every
*"not decompiled"* / *"not traced"* / *"needs a Ghidra task"* with either the finding or
**"not read"** — which is an honest backlog item rather than a false blocker. And a standing
rule: *a claim that something is expensive gets a timestamp, because tooling changes and the
claim does not expire on its own.*

---

### F3 — 651 inferred names read as fact inside the decompilation, because `ApplySymbols` does not carry the confidence label that the docs are so careful about.

The project's discipline about `[V]` versus `[D]`/`[I]` is genuinely good in the documents —
`docs/battle.md` carries 111 `[V]` against 47 `[D]`/`[I]`, `docs/kingdom.md` 101 against 31,
and `docs/battle-ai.md` opens by declaring itself mostly `[D]`. `docs/symbols.json` tags
every entry: **352 verified, 42 inferred.**

That discipline stops at the tool boundary. `decompile-all.ps1` runs `ApplySymbols` first,
and its own header explains why that matters:

> Naming one global makes dozens of unrelated functions legible, and this is how that
> benefit reaches code nobody has opened yet.

Exactly so — and an *inferred* name is amplified by the same mechanism. Measured over the
whole tree:

```
symbols.json:  352 verified, 42 inferred
inferred entries that are data globals:            15
occurrences of those names in tools/oracle/decomp: 651
   249  g_screenStride         0x004EB274
   155  g_deterministicBattle  0x00553030
    61  g_siegeApproachScore   0x00553FB0
    40  g_debugSelectedMan     0x00553E68
    31  g_turnPhase            0x00569584
    30  g_siegeBreachScore     0x0053E990
```

Decompiled *functions* do carry their confidence — `Weather_UpdateAll`'s body opens with
`/* [verified] … */`. Globals carry nothing. So a reader of `00410000.c` sees
`if (g_deterministicBattle != 0)` 155 times and has no signal that
`docs/symbols.md:465` ends its entry with *"which is a reading, not a proof"*.

`battle-ai.md` §9 is right to keep flagging `g_siegeApproachScore` and `g_siegeBreachScore`
as `[I]`. But the flag lives in a document, and the name lives in 91 places in the artefact
people actually read. This is the structural version of C13 — *the artefact an agent leaves
behind is evidence; its prose is a claim* — with the roles reversed: here the artefact
carries the claim and the prose carries the caveat.

**What should change.** `ghidra_scripts/ApplySymbols.java` should mark inferred symbols in
the database — either a name suffix (`g_deterministicBattle_i`) or, less invasively, a
plate comment the way function comments already work. It is a few lines in one file plus
one re-run of `decompile-all.ps1`. Until then, no agent reading the tree can tell the 352
from the 42.

---

## 1. Every "blocked / impossible / needs X" claim, audited

### Settled by this audit

| Claim | Where | Verdict |
|---|---|---|
| `Path_Search` visit counters *"Not resolvable from the decompilation alone — it needs the callee's signature"* | `decisions.md` Open questions | **False, and the answer is the bad one.** F1. |
| `Battlefield_BuildCastle` / `Battlefield_BuildRandom` / `Path_SearchSiege` *"not decompiled"* / *"not traced"* | `battle-ai.md:756,781`, `battle.md:199` | **False since commit `9f97838`.** All three in `decomp/00470000.c`. F2. |
| *"Running Ghidra would have contended for the project lock"* | `audit.md` §8 | **Moot.** Re-verification is now `rg`, not a Ghidra run. F2. |
| *"The fourteen AI turn handlers … none decompiled"* | `kingdom.md:1070` | **Contradicted by `kingdom.md:240` in the same file**, which says all fourteen are decompiled. One of the two is stale. |
| `WEATHER_JITTER_BOUND` *"is **invented** … It needs tracing before any weather behaviour is trusted"* | `decisions.md` Open questions | **Already resolved and the decision log is stale.** Independently re-derived here: `Rand_Advance` (`0x00404A46`) publishes `DAT_0058fd60 = g_randStateB & 0x7f`, and `Weather_UpdateAll` divides it by 8 via the signed-division idiom `(x + ((x>>31)&7)) >> 3`. Range 0…127, jitter 0…15. `crates/l2-kingdom/src/weather.rs:89` has 128 and is right. |
| `local_modifier` *"Never traced … Zero until somebody reads it out of the binary"* | `crates/l2-kingdom/src/weather.rs:104` | **False. It took four greps.** See F5 below. |
| `Title.pl8` *"decodes with provably correct geometry, but no shipped `.256` colours it"* | `formats/pl8.md:192` | **The wrong question.** `Lords2.exe` contains **zero** occurrences of `title.pl8` or `title.256`; the only title string in either shipped executable is `imptitle.smk`, twice. `mapl2.exe` contains no `title` string and no `.256` string at all. No shipped code pairs `Title.pl8` with any palette, because no shipped code loads `Title.pl8`. F7. |
| `g_goodsStock` *"Reads like the quantity a merchant carries"*, marked inferred | `symbols.md:576` | **Zero references in the entire binary.** F6. |
| `g_deterministicBattle` *"flag was not traced to where it is set"* `[I]` | `battle-ai.md:734` | **A write exists** at `decomp/00400000.c:7904`. The *meaning* is still a reading — `symbols.md:465` says so correctly — but "not traced to where it is set" is no longer true. |

### Standing, with the specific test that would settle each

| Claim | Where | The test |
|---|---|---|
| *"A fullscreen DirectDraw game generally captures as pure black, so the screen cannot be read either."* | `decisions.md` D8 | **Never measured against this game.** I grepped the whole `docs/` tree: no document records a capture attempt on `Lords2.exe`. The word *"generally"* is doing all the work, and the conclusion it supports is quoted in three places as a project blocker. See §2. |
| *"NAT — Not tested, and not testable on one machine."* | `netcode.md:42` | **Correctly scoped and correctly labelled.** This is the discipline working: the mechanism is named (loopback traverses nothing), the honest status is recorded as "unknown", and the doc explicitly warns that `tests/resilience.rs` must not be read as covering it. Two machines on different networks. Nothing cheaper exists. |
| *"`blowUsed` may never be cleared outside `Melee_Tick`"*, writers *"were not traced"* | `battle.md:472` | Now a grep: `rg "0x18C\|blowUsed" tools/oracle/decomp/`. Whole-binary write-site enumeration for a struct offset is text search over `+ 0x18c`, which the tree supports. |
| *"the per-battlefield engagement budget at `0x00553080` … was not traced to where it is filled"* | `battle-ai.md:774` | `rg "553080" tools/oracle/decomp/`. Same shape as F6, which took one command. |
| *"Frame rate … cannot be converted to seconds"* | `battle.md`, `battle-ai.md` §9 | The only genuinely runtime-shaped item in the battle backlog, and the honest one. The static half — where the frame counter is incremented and what gates it — is readable; the wall-clock half is not. Worth splitting into two claims, because half of it is free. |
| The `.skr` terrain byte → name mapping *"needs `mapl2.exe`'s palette-button order, which is a Ghidra task"* | `audit.md:88` | Fair, but note `mapl2.exe` is a *different binary* and `docs/agents.md` already documents the separate-project workflow for it. It is a task, not a blocker. |

---

## 2. D8, C14, C15, C16 — the pattern, and what is left of D8

The four of them tell one story and it is worth stating plainly, because the same error is
about to be made a fifth time.

D8 listed three blockers to driving the original. **C15 killed the first** (`AttachThreadInput`
hands over the foreground; the constants were read from a live process). **C16 killed the
motivation** (the values were in `.text` all along; the live read was never needed). The
third blocker — *"a fullscreen DirectDraw game generally captures as pure black"* — has
never been tested, and it is the one still quoted:

* `battle.md:1109` — *"Nothing has been compared against the original's framebuffer … D8
  blocks driving the original's UI."*
* `audit.md` §8 — visual claims not re-checked because *"Repeating them means launching the
  game, which `agents.md` and `decisions.md` D8 both say to avoid."*
* `native/ddraw-proxy/ddraw_proxy.cpp:8` — *"a fullscreen DirectDraw game captures as black.
  Injection needs neither."*

Three things make this claim weaker than its use:

1. **The premise is about *fullscreen*, and the game has been run windowed.**
   `docs/shots/game-live.png` is a screenshot of `Lords2` in a normal Win32 window with a
   title bar, running under DxWnd. That capture succeeded.
2. **`tools/screen.ps1` already implements the documented workaround** and says so in its own
   header: `-Method screen` is *"needed for some DirectDraw/GPU surfaces that refuse to
   render via PrintWindow."* A capability exists that nobody has pointed at the game.
3. **The proxy makes screen capture the wrong technique anyway.** `native/ddraw-proxy/` is
   built (`ddraw.dll` present) and installed into the hard-link sibling
   `F:\games\lords2-instrumented`. It is 112 lines that forward `DirectDrawCreate` and log.
   A proxy that also wraps the returned interface sees `Lock` / `Blt` / `Flip` on the primary
   surface — **the framebuffer arrives as a pointer inside the process**, with no focus, no
   screen grab, and nothing a screen lock can spoil. That is D8's own conclusion (*prefer
   injection*), and the framebuffer half of it was never built. The proxy's header comment
   currently cites the unmeasured black-screen claim as its justification, which is
   backwards: the proxy is what makes the claim irrelevant.

**And there is an artefact nobody can account for.** `docs/shots/start.png` is a **640×480**
image of the original's title screen, correctly coloured. It did not come from our viewer —
`l2view.png` and `map0.png` are 656×519, our `winit` window with chrome. It did not come from
our PL8 decoder either, because `formats/pl8.md:192` states no shipped palette colours
`Title.pl8`. No document in the repo records where it came from. So either D8's third blocker
is false, or the `Title.pl8` palette entry is stale — and nobody has checked which. That is a
process finding independent of the answer: **an artefact in the repo that appears to
contradict three documents, with no provenance recorded anywhere.**

**What should change, in order.** (a) Extend the ddraw proxy to capture the primary surface —
this is the technique D8 itself argued for, it needs no focus, and it makes the black-screen
question moot rather than answered. (b) Record `start.png`'s provenance or delete it. (c)
Stop citing D8 as a blanket blocker: **two of its three legs are gone and the third was never
weighed.** The honest summary of D8 today is *"driving the UI is difficult and unnecessary,
because injection reaches further"* — which is a much narrower claim than the one three
documents are leaning on.

---

## 3. Toolchain

### What is duplicated (measured)

```
ghidra_scripts* + tools/**/*.java :  46 files, 31 distinct MD5 hashes  -> 15 exact duplicates
per-task out/ directories        :  7 dirs, 247 files, 6.5 MB
whole-binary decompilation       :  13 files, 2,452 functions, 3.3 MB
```

The largest hash groups: `RefsTo.java` exists identically in three script dirs;
`DecompileFunc.java` in three; `BDecomp/BDump/BRefs/BCallArg` are duplicated between
`ghidra_scripts_battle` and `ghidra_scripts_battleai`; `ghidra_scripts_view/VB*.java` differ
from the battle family only by class name (Ghidra requires class name = file name);
`tools/kingdom2/ghidra/K*.java` are byte-identical to `tools/kingdom/ghidra/K*.java`. Five
`ghraw.ps1` copies differ only in project name and script path.

Most of that is forced by two real constraints — Ghidra's class-name rule and the
one-process-per-project rule in `docs/agents.md` — so it is not waste so much as an unpaid
abstraction. The 6.5 MB of superseded per-task `out/` fragments *is* waste, and they are the
fragments `decompile-all.ps1`'s own header was written to replace. They are gitignored, so
the cost is not repo weight; the cost is that an agent grepping `tools/` gets stale C
alongside the current tree.

### What `decomp/` cannot answer, and what that should cost

`DecompileAll.java` calls `decompileFunction()` and writes `.getC()`. It emits **no raw
bytes, no disassembly, no xrefs, no data-section dump, no string table.** That is why the
`DumpBytes` / `Disasm` / `FindScalar` / `ScanRange` / `XrefData` scripts still exist.

Two of those gaps do not need Ghidra at all:

**Raw bytes and data.** `tools/audit/rva.js` reads any VA out of the file in **44 ms**. It
subsumes `DumpBytes.java`, `BDump.java`, `KDump.java` and their duplicates for every address
that is stored in the image, and it also prints the exact boundary that C14 and C16 are
about:

```
$ node tools/audit/rva.js sections                                  (44 ms)
  .text    VA 0x401000..0x4cfb06   vsize 846598   raw 846848
  .rdata   VA 0x4d0000..0x4d10f8   vsize 4344     raw 4608
  .data    VA 0x4d2000..0x5cee60   vsize 1035872  raw 80384
                                   [955488 bytes mapped but NOT stored]
```

That last line is C14's "tier two" stated as a number. Everything below `0x004E5A00` in
`.data` is in the file; everything above it is uninitialised and reads as zero. The script
prints which side of that line an address falls on before it reads, so the C14 failure mode —
a file read confidently returning zero — announces itself.

**Xrefs to named globals.** The claim that xref enumeration needs live Ghidra is *mostly*
false now, and I measured it: of the **164 named data globals in `symbols.json`, 162 are
reachable by plain `rg` over `decomp/`** — 98.8%. That is because the binary is non-PIC and
`ApplySymbols` has already replaced the addresses with names. The two exceptions are
`g_stateArrayPtrs` (documented as having no reader) and `g_goodsStock` (F6, which has none).
So `RefsTo.java` and friends are worth keeping for *unnamed* addresses and for data-borne
references, and are not worth starting Ghidra for a named global.

**The one genuine gap is disassembly.** No `objdump`, `llvm-objdump`, `ndisasm`, `dumpbin` or
`radare2` is on this machine (checked), and `docs/environment.md` is right that Python is
absent — `python3` resolves to the Microsoft Store stub and prints *"Python was not found"*.
So instruction-level questions still need Ghidra. They are rarer than they look: `rva.js`
plus a byte pattern handled every instruction-level question in this audit, including the one
in F5 that the decompiler rendered ambiguously.

### The oracles disagree with each other about what an oracle is

| script | asserts? | coverage |
|---|---|---|
| `tools/oracle/kingdom.ps1` | **yes** — 19 tables, pass/fail, non-zero exit | 19 tables, expected values hand-typed |
| `tools/oracle/initconsts.ps1` | **yes** — 10 known + 5 unnamed, pass/fail, non-zero exit | the `Rules_InitConstants` writes |
| `tools/oracle/tables.ps1` | **no** | prints three `l2-sim` tables for a human to look at |

I ran the first two: `kingdom.ps1` reports **19 passed, 0 failed**; `tables.ps1` prints three
tables and exits 0 unconditionally. So the crate whose tables are *not* asserted is `l2-sim`,
the battle simulation — the one with the most numbers and the one C12's wrong claim reached.
Its header says *"Every number in `crates/l2-sim` came out of a decompiler listing … This
reads the tables straight out of `Lords2.exe` instead"*, which is true of what it *reads* and
not of what it *checks*. Nothing fails if `l2-sim` drifts.

`kingdom.ps1`'s coverage is also narrower than it appears: it checks
`g_armyHappinessCost[0..31]` against a table the crate declares with **102** entries. I read
all 102 out of the binary and diffed them — they match, so nothing is wrong today, but 70 of
them are unguarded.

**`tools/audit/tablediff.js` is the fix, and it took 81 ms to prove.** Most crate tables
already name their source address in the doc comment above them, so nothing needs typing
twice:

```
$ node tools/audit/tablediff.js                                     (81 ms)
  PASS  AI_GOLD_GRANT         0x4dc1e0   20 x i32
  PASS  AI_GOLD_GRANT_SMALL   0x4dc230   20 x i32
  PASS  ARMY_HAPPINESS_COST   0x4d8778  102 x i32
  PASS  BIRTH_RATE_LADDER     0x4d6308   40 x i32
  PASS  CASTLE_TAX_BONUS_PCT  0x4d8a28    6 x i32
  PASS  HEALTH_BAND_LADDER    0x4d6520   10 x i32
  PASS  HEALTH_DELTA          0x4d64a8   30 x i32
  PASS  HEALTH_HAPPINESS      0x4d6548    5 x i32
  PASS  HERD_WEATHER_PCT      0x4d6560    6 x i32
  PASS  RATION_TABLE          0x4d6738   12 x i32
  PASS  WEAPON_COST           0x4d8990   12 x i32

11 pass, 0 fail, 11 compared; 18 tables had no usable address in their doc comment
```

Eleven tables verified with **zero** hand-typed expectations, in a twelfth of a second, and a
table added tomorrow is covered the moment its doc comment names an address. The eighteen it
skips are skipped for a reason worth knowing: `CASTLE_COST`, `CASTLE_WORKFORCE`,
`CASTLE_GARRISON_CAP`, `CASTLE_FREE_ARCHERS` and `GOOD_SELL_PRICE` *are* checked by
`kingdom.ps1`, so their addresses exist and just aren't in the doc comment; while
`DEATH_RATE_BY_SEASON`, `DRYNESS_BY_SEASON`, `DRYNESS_LADDER`, `SCORE_INPUT_OFFSETS`,
`AI_FIELD_LADDER` and `FACING_DELTA` are code immediates or genuinely unlocated. **The gap
between those two groups is exactly the list of tables nobody can currently check.**

---

## 4. What is genuinely unverified, ranked by risk

The `[V]`/`[D]`/`[I]` discipline holds well in the documents. `docs/audit.md` is a real
adversarial re-derivation and found 23 genuine problems in other docs. The failures below are
not failures of labelling — they are things labelled correctly and then not acted on, plus
two things labelled better than they deserve.

**Structural — a wrong assumption here is expensive to unwind.**

1. **The `Path_Search` visit-counter divergence (F1).** Shipped code, deterministic, affects
   36% of every non-skirmish battlefield. Not a constant; a behaviour.
2. **`local_modifier` is stubbed to zero and the real function is four greps away (F5).**
   `crates/l2-kingdom/src/weather.rs:104` returns `0`. `FUN_00449D6E`
   (`decomp/00440000.c:5806`) returns, on county field `+0x21E`:
   *Summer* — band 0 → **+4**, 1 → **+2**, 2 → **−8**, 4 → **−12**;
   *Winter* — band 1 → **−2**, 2 → **−4**, 3 → **−6**, 4 → **−10**; zero otherwise.
   The band is fixed at new-game by `FUN_00451150` from the county index (1–3 → 0, 4–5 → 1,
   6–9 → 2, 10–11 → 3, 12+ → 4), so it is a permanent per-county climate. The seasonal
   deltas are 8/24/12/−12, so a −12 modifier is not a rounding difference — it can invert a
   season. **Every county's local weather swing in our engine is currently wrong.**
   One caveat, and it is why I read the bytes: the decompiler shows `== 4` **twice** in the
   Summer chain, with band 3 unhandled. The instruction stream confirms it —
   `83 f9 04` at both `0x449E06` and `0x449E2C` (`node tools/audit/rva.js 449d6e 360`) — so
   the `−24` arm is **dead code in the shipped binary** and band 3 really does fall through
   to zero. That is a bug in the original, almost certainly a mistyped `case 3`, and a
   reimplementation would "fix" it by accident.
3. **The RNG model.** `Rand_Advance` (`0x00404A46`) is not a function consumers call. It steps
   two 31-bit LFSRs thirty-one times and **publishes six masked globals** — `&0x7FFF`, `&0x7F`
   and `&7` from each — which then stay fixed until the next advance. `Weather_UpdateAll`
   reads two of them (`DAT_0058FD60` for the jitter, `g_rand7A` for the county) from a
   *single* advance. `crates/l2-kingdom` draws twice from one `Pcg32`.
   `crates/l2-kingdom/src/weather.rs:51` acknowledges this and its reasoning is sound for
   weather (*the range is what a rule turns on*). What is **not** established anywhere is how
   often `Rand_Advance` is called — I count nine call sites across the binary — and that is
   the fact a save-state-level differential test turns on. `docs/netcode.md` D-3 freezes our
   PRNG for lockstep; it does not claim to match the original's, and no document says which
   guarantee is intended.

**Cheap — a wrong constant, easily fixed, but currently unguarded.**

4. **`l2-sim`'s battle tables are printed, not asserted** (§3). The melee attack column does
   match `g_meleeAttackTable` as printed, but nothing enforces it.
5. **70 of `ARMY_HAPPINESS_COST`'s 102 entries are unchecked** by `kingdom.ps1`. I verified
   all 102 against the binary: correct today.
6. **The eighteen crate tables `tablediff.js` cannot reach** (§3), of which about six have no
   located address at all.

**Named on plausibility alone — C3's exact shape.**

7. **`g_goodsStock` (`0x004D8950`) (F6).** Marked `inferred` in `symbols.md:576` and described
   as *"reads like the quantity a merchant carries"*. I read it out of the file: sixteen i32s,
   `0, 1000, 100, 200, 100, 500, 100, 100, 200, 500 × 6, 0`. Then I searched the entire
   decompilation for any reference — by name and by address:

   ```
   $ grep -c "d8950" tools/oracle/decomp/*.c
     ZERO references across all 2452 functions
   ```

   Its neighbour `g_goodsPrice` at `0x004D8910` has exactly one reader (`FUN_0042847C`, which
   copies sixteen dwords into a record array and stops). So `g_goodsStock`'s entire identity
   is that its values look like stock quantities next to a table that looks like prices. That
   is N things matching N other things. It should be marked *"no reader in the binary"*, which
   is a stronger and more useful statement than *"inferred"*.
8. **`Title.pl8`'s palette (F7).** Not a wrong claim, a wrong question — see §1.
9. **651 inferred-name occurrences in the decompilation (F3).**

---

## 5. Method changes, in priority order

Each is stated with the measurement that justifies it.

**1. Sweep the four "What is still unknown" sections against `decomp/index.txt`.**
`node tools/audit/coverage.js check <names…>` answers *"is this function readable?"* in
55 ms. Three functions that documents call undecompiled are present, one of them carrying
*"every siege claim in §6 rests on that"*. Replace *"not decompiled"* with *"not read"*
everywhere, and date every cost claim so it expires visibly.

**2. Teach `ApplySymbols` about confidence.** 352 verified and 42 inferred symbols go into
the database indistinguishably; 651 occurrences of inferred global names now read as fact in
the artefact agents actually grep. Functions already get a `/* [verified] … */` plate; globals
get nothing. A few lines in one Java file plus one re-run.

**3. Make `tables.ps1` assert, and replace hand-typed expectations with `tablediff.js`.**
`kingdom.ps1` asserts 19 tables and exits non-zero; `tables.ps1` asserts nothing and exits 0
whatever it reads. `tablediff.js` verifies 11 tables in **81 ms** with no expected values
typed anywhere, by reading the address out of the crate's own doc comment. Adding the missing
addresses to the six doc comments that lack them would take the count past 17 for no ongoing
maintenance at all.

**4. Adopt `rva.js` as the default for byte-level questions.** 44 ms, no Ghidra, no project
lock. It settled a decompiler ambiguity in F5 that `decomp/` alone could not, and it prints
the stored/not-stored boundary that C14 and C16 are both about. Reserve Ghidra for what it
uniquely has: disassembly, xrefs to unnamed addresses, scalar/operand search. Note that xrefs
to *named* globals are not in that set — 162 of 164 are reachable by `rg`.

**5. Extend the ddraw proxy to capture the primary surface, and stop citing D8's third leg.**
The proxy is built and installed; it is 112 lines. Wrapping the returned interface gives the
framebuffer as a pointer, inside the process, with no focus and no screen grab. This is what
D8 concluded and it was never built, while the unmeasured black-screen claim it was meant to
sidestep is quoted as a blocker in three documents and in the proxy's own header comment.

**6. Trace the remaining "not traced" items by grep before commissioning any agent for them.**
`local_modifier` took four commands and turned out to be shipped-code-affecting.
`g_goodsStock` took one and inverted the finding. The engagement budget at `0x00553080` and
the `blowUsed` writers are the same shape. **Prior art first (C5), then the decompilation
second, and only then a new investigation** — the middle step is new and it is nearly free.

**7. Delete or archive the seven superseded `out/` directories** (6.5 MB, 247 files). They are
the fragments `decompile-all.ps1` was written to replace and they still answer `rg` queries
over `tools/`. `tools/battleai/out/path.c` is cited by C13 as evidence, so archive rather than
delete, or move the citation to the tree.

---

## 6. What I did not check, and why

* **I did not launch the game.** The brief allows it with a measured reason. I found one
  candidate question — whether the original's framebuffer captures — and then found that the
  right technique for it is the ddraw proxy rather than a screen grab, so a launch would have
  measured the wrong thing. DxWnd, under which the existing screenshots were taken, is not on
  this machine (`E:\dev\tools` holds only Ghidra), so a launch would also have been fullscreen
  and would have taken over the user's display. Nothing was started; nothing is running.
* **I did not re-time `decompile-all.ps1`.** The "22 seconds / 2,452 functions / 3.0 MB"
  figure is quoted from the project. I verified the *output*: 13 files, 2,452 index entries,
  3.3 MB, stamped 21:14 today, with all 230 function symbols from `symbols.json` correctly
  applied. I did not re-run it, so the 22 s is the project's number, not mine.
* **I did not verify `Imptitle.smk` actually colours `Title.pl8`.** F7 shows the executable
  never names `Title.pl8`; testing whether the Smacker video's own palette matches it is the
  obvious next step and `tools/media/smkinfo.js` already exists, but that is a format
  question, not a method one.
* **I did not audit `docs/netcode.md`, `docs/modding.md` or `docs/formats/` line by line.** I
  swept them for blocked-style claims and spot-checked `netcode.md`'s NAT entry, which is the
  best-scoped unknown in the tree. A full pass over the format documents is what
  `docs/audit.md` already is.
* **I could not establish `docs/shots/start.png`'s provenance.** I established what it is
  *not* (not our viewer — wrong dimensions; not our PL8 decoder — the palette is documented as
  missing) and that no document records it. Saying which of the two contradicted claims is
  false would be fitting a story to evidence, which is C3.
