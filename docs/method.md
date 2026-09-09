# Method

How to work on this project, and why. `decisions.md` records *what* was decided and where
we were wrong; this records *how to decide*, so the same class of mistake stops recurring.

It exists because of a pattern in the correction log: the expensive failures were not bad
execution, they were **good execution of the wrong technique**, and in both recent cases the
error was caught by the user rather than by whoever was doing the work.

---

## 1. The cost table

Measured, not estimated. Estimates are how we got here.

| technique | cost | what it answers |
|---|---|---|
| `rg` over `tools/oracle/decomp/` | **instant** | any question about code: who reads a global, what writes a constant, which functions form a subsystem |
| Read a table from the file (`tools/oracle/tables.ps1`) | **instant** | any initialised data — proven byte-identical to live memory |
| Recover init constants from `MOV` immediates (`initconsts.ps1`) | **instant** | anything `Rules_InitConstants` writes |
| Whole-binary decompile (`decompile-all.ps1`) | **22 s** | rebuilds the grep corpus; re-run after any `symbols.json` change |
| A corpus test over shipped data | seconds | whether a format claim holds for *all* files, not one |
| Targeted Ghidra script | ~1 min | only what the decompiled corpus cannot express |
| A subagent | **~40 min, ~400k tokens** | a whole subsystem, end to end |
| Launching the game | minutes, needs focus, can be lost to a screen lock | **almost nothing.** See §3 |

The two numbers that should shape every decision: **the entire binary decompiles in 22
seconds**, and **a subagent costs 40 minutes**. Anything answerable by `rg` should never
become a task.

---

## 2. Order of attack

Work down this list. Stop at the first step that answers the question.

1. **`rg` the decompiled corpus.** It is 2,452 functions and it is already on disk.
2. **Read the data from the file.** No process, no focus, nothing to clean up.
3. **Read the code that produces the data** — the lesson of C16. If a value is absent from
   `.data`, the function that writes it has the immediate.
4. **Write a self-verifying test** — a property of the *data*, not of our code. The PL8
   end-offset invariant found 153 broken files with no oracle involved; it remains the
   single highest-yield check in the project's history.
5. **Targeted Ghidra script**, for what the corpus genuinely cannot express.
6. **A subagent**, for a whole subsystem with a clear boundary.
7. **Run the original.** Last. See below.

---

## 3. What running the game is actually for

Almost nothing, and this took far too long to establish.

- Logic and control flow → the decompiler.
- Initialised tables → the file. Disk and live memory are byte-identical, verified.
- Runtime-written constants → the `MOV` immediates in the writing function (C16).

The **only** thing a running original can supply is whether hundreds of individually
correct functions were *assembled* correctly — an emergent property no listing contains.
That is real, but it is one question, and it is not the second thing to reach for.

And it is worth remembering that the historic misreadings — C1, C3, C5, C7 — were *not*
caught by running the game. They were caught by corpus checks and invariants.

---

## 4. Failure modes, from our own log

Each of these has cost real time here.

**Asserting a cost or an impossibility without measuring it.** C16 (a live process is
"required"), and the per-question Ghidra habit (analysis is "the expensive part"). Both
false, both settled in under a minute once anyone actually checked. **If you write "this
needs X" or "this is blocked", record the measurement in the same sentence.**

**Generalising a true observation one step too far.** D8 observed correctly that
`SetForegroundWindow` fails from a background process, and concluded UI automation was
impossible. `AttachThreadInput` fixes it (C15). *"It fails" and "it fails the way I called
it" are different claims.*

**Fitting a plausible story to correct output.** C3 — three blitters matched to three
storage modes, a coincidence that assembled itself into a theory. Decompiler output invites
this. If N things match N other things, that is not evidence.

**Believing one working example.** C1 — "the format is fully decoded" after one sprite;
153 of 291 files were failing. Validate across the corpus, always.

**Trusting a document over the bytes.** C8 and C10 — public format documentation wrong in
three places, the printed manual wrong twice. Prior art tells you *what to look for*, which
is valuable, and it is never the authority. This includes our own docs.

**An absence quoted without its denominator.** "No function in the binary references `L2.eng`
group 62" was written here as a finding. Re-deriving it turned up the number that mattered:
**135 of the 317 groups are unreached by a literal**, several of them demonstrably live text
reached through a computed id. The observation was true and it was ordinary. An absence is
only evidence in proportion to how surprising it is, so **a negative result must be reported
with the size of the set it came from** — and a control helps: asking the same query for the
group that *does* ship named its function immediately, which is what showed the query worked
at all.

**A tool that is wrong is worse than an analysis that is wrong**, because every agent
inherits it and their agreement then looks like corroboration. `anchor.js`'s `litNum` parsed
`'\b'` as the letter *b*, and produced six screen ids that did not exist, self-consistently,
joined across three dispatchers. It was caught by an existing document, not by the work
using it. Two consequences worth keeping: **re-derive anything load-bearing that came out of
a tool, by a route that does not use the tool**; and when you write a parser for the
decompiler's output, make the unhandled case *report itself* rather than silently returning
nothing — the re-derivation above prints its count of unparsed literals precisely so that a
missed escape form shows up as a loud zero-or-not rather than as a quiet absence.

**A test whose name claims more than its body checks.** C12 —
`expensive_ground_is_deferred_rather_than_weighted` asserted only that a path crossed a gap,
which was true under either reading, so it passed before and after a semantic change. **A
test that cannot fail is not a test.**

---

## 5. Running subagents

At ~40 minutes and ~400k tokens each, they are the most expensive tool here, and worth it
only for a whole subsystem with a clear file boundary.

- **They must commit their own work**, in focused commits with explicit paths, never
  `git add -A` (C9). Telling three agents *not* to commit produced exactly one bad outcome:
  all three died on the same watchdog stall and left a tree that would not compile with no
  checkpoint to fall back to.
- **They must keep the workspace compiling** — `cargo build --workspace` after each coherent
  chunk. If one dies mid-edit, the difference between a broken tree and a smaller one is
  whether the last thing it did built.
- **Give them the cost table.** Two agents independently re-ran Ghidra for things `rg` now
  answers instantly.
- **Their prose is a claim; the artefacts they leave are evidence** (C13). When an agent
  reported the pathfinder finding, the right move was neither to accept nor dismiss it but
  to open the raw decompiler output it had left behind — which said something *stronger*
  than its summary did.

---

## 6. When to re-evaluate

Not on a schedule. On these triggers:

- **A task is taking longer than its cost-table estimate.** That usually means the technique
  is wrong, not that the task is hard.
- **About to run the game, or spawn an agent.** The two most expensive actions. Ask what
  cheaper step was skipped.
- **About to write "blocked" or "not possible".** Record the measurement, or do not write it.
- **A new capability lands.** The whole-binary corpus made several existing tools redundant
  the moment it existed; nobody would have noticed without asking.

---

## 6a. Before building anything, ask whether the original already answers it

The rule that would have prevented C21, stated as a check rather than a virtue:

> **Is there code in `Lords2.exe` that does the thing I am about to write? Have I read it?**

For a format, a rule or an algorithm this is second nature here. It was never applied to a
*screen*, because no phase was named after screens, and a whole layer got invented while
every layer beneath it was verified to four decimal places.

It costs one `rg` over `tools/oracle/decomp/`. If the answer is "yes and no", the task is not
"build X" — it is "read the original's X, then build it", and those are different tasks that
produce different work.

The corollary for delegation: **a task worded "build a screen" will get you a built screen.**
Word it "decompile the screen, then build what the decompilation says" and say which is the
deliverable if the two run out of time.

## 7. Naming functions: two mechanical filters

`tools/oracle/anchor.js`. Both filters answer *what a function is about*, cheaply and without
judgement. Neither answers *what it is*, and the gap between those two is where the errors
live — measured below, because guessing at it is how C3 happened.

### 7.1 Record-stride arithmetic

The binary indexes every record array by a fixed stride: `0x300` county, `0x1A4` unit, `0x160`
realm, `0x1B0` battle figure, `0x34` battle unit. A function computing `[base + i * 0x300]` is
touching counties **even when `base` is an unnamed `DAT_`**, which is the usual reason a
function looks dark.

    node tools/oracle/anchor.js stride --unnamed

The filter is essentially exact about the claim it makes: across **7,723 occurrences** of
those five constants in the corpus, **zero** appeared outside array-index context — the
multiplication only ever indexes a record. And each stride is dominated by one base region
(81–99.8%), so it identifies the record *type* reliably. What it does **not** do is
distinguish the primary array from a parallel array of the same stride; for that you still
read the base.

**§7.5 has since retired most of this filter's work, which is the point of it.** Once the
county, unit and realm arrays have a struct type the decompiler folds the arithmetic away,
so the stride stops appearing — and a function that no longer needs the filter is a function
that now says what it does:

| stride | functions carrying it, before → after | of those, unnamed |
|---|---|---|
| county `0x300` | 297 → **38** | 178 → **22** |
| unit `0x1A4` | 173 → **37** | 117 → **24** |
| realm `0x160` | 190 → **38** | 119 → **10** |
| battle figure `0x1B0` | 146 → **4** | 90 → **1** |
| battle unit `0x34` | 81 → **0** | 13 → **0** |
| **total** | 692 → **115** | 399 → **56** |

The residue — 38 functions still doing county arithmetic by hand — is not a failure of the
struct. Those are the places the decompiler cannot fold: whole-record copies, pointer walks
that step by the stride, and code that takes the address of a record and passes it on. They
are a short, concrete list rather than a third of the binary.

### 7.2 The `L2.eng` string ids are a confession

Five functions take a literal `(group, index)` pair — `Eng_DrawString` (`0x00402D37`),
`Eng_Seek` (`0x004018D7`), `Eng_CopyString` (`0x004017BF`), `Ui_DrawCentred`, and the
word-wrapper `FUN_0040328E`. **Index 0 of every group is a descriptive label**, so the game
ships a one-line description of each of its 317 string groups. Look the call site up and the
function names its own subject.

    node tools/oracle/anchor.js strings --unnamed
    node tools/oracle/anchor.js strings --group 80

Two multipliers on top:

**Sound files are named after the group.** 197 `sNNN_MM.wav` files ship, and `NNN` is the
`L2.eng` group — `s080_01.wav` plays under the screen that draws group 80. Only 15 distinct
references survive in the corpus, so this corroborates rather than finds.

**The screen-id dispatch is a third table for free.** `Screen_Draw`, `Screen_DrawWidgets` and
`Screen_HandleInput` each switch on `g_screenId`; `anchor.js screens` joins them, so a painter
can be corroborated by appearing in two or three of the three. That is what placed nine of
this batch's candidates without reading a line of their bodies.

### 7.3 The measured error rate — the number to actually use

A batch of 19 unnamed functions was selected by filter 2 and each one checked against
something that could have failed (callers, the strides it indexes, the constants it reads,
whether the strings it draws in order match the flow claimed). Two very different rates fell
out, and conflating them is the trap:

| the claim | correct | rate |
|---|---|---|
| **filter output** — "this function's subject is what group *N* is about" | 17 / 19 | **89%** |
| **hypothesis on top of it** — "therefore it is the *X* screen" | 11 / 19 | **58%** |

**The filter is reliable about subject and unreliable about role.** Seven of the nineteen were
about the right thing and doing something else with it: three group-71 functions were castle
*status blocks* in other panels rather than the castle-building screen; the group-77 candidate
was the herd panel, not the sowing forecast; two group-11 candidates were skirmish setup, not
siege messages.

Two specific lessons worth more than the percentages:

- **The two outright subject failures were both `Eng_Seek`, not `Eng_DrawString`.** `Eng_Seek`
  leaves a pointer for the caller to *copy*, so its callers **write** `g_playerNames` rather
  than display it — `Realms_AssignLords` assigns lords, it does not show them. Treat a draw
  call and a seek call as different evidence.
- **Read index 0, not the strings a summary quotes.** The group-11 candidates were described
  from string 1, *"The siege is on"* — which is the game's **subtitle**. Index 0 is "Lords of
  the Realm 2" and the group is the front end. The filter was right and the reading of it was
  wrong.

C25 came out of this batch and is the best argument for the technique: the game's own English
overturned a **[V]**-marked tile-flag claim that our own consistent mislabelling had preserved
for months.

### 7.4 "844 dark functions" was never true

The figure has been repeated as though that many functions were unanalysable. Run
`node tools/oracle/anchor.js dark`:

| | count | median size | |
|---|---:|---:|---|
| touch **no global at all** | **291** | **22 b** | the only genuinely dark ones — and 175 are ≤ 40 b, so accessors and thunks |
| touch only *unnamed* globals | 478 | 97 b | anchored the moment one global is named |
| touch a *named* global | 968 | 238 b | already in a cluster |

The bound on how much of the binary reads as prose is the **global** ratio, not the function
count: 2,898 distinct globals, 303 named.

And even that overstates it. `node tools/oracle/anchor.js fields` resolves every `DAT_` that
appears beside a record stride back to `record[i] + offset`. Ghidra applies a name to one
address, so `g_counties` labels `0x0053F9B0` and every field of every county used to invent
its own `DAT_`: `DAT_0052F218` was not an unknown global, it was `unit[i] + 0x168`, the
total-men field `docs/armies.md` has documented all along. **334 of the "unknown globals"
were field offsets of three arrays that were already named.**

### 7.5 So the arrays were given a struct type, and it is the largest single move so far

`docs/records.json` is the layout of every fixed-stride record array, and
`ghidra_scripts/ApplyRecords.java` applies it before every corpus rebuild — see
`tools/oracle/decompile-all.ps1`, which now runs `ApplySymbols` then `ApplyRecords` then
`DecompileAll`. One edit to the JSON reaches the whole corpus.

Measured on the same 2,452 functions, before and after:

| | before | county/unit/realm/lord | + the two battle arrays | + the two grids |
|---|---:|---:|---:|---:|
| `DAT_` occurrences in the corpus | 24,608 | 19,661 | 17,192 | **15,308** |
| distinct `DAT_` names | 2,977 | 2,575 | 2,461 | **2,309** |
| distinct globals the corpus sees | 3,302 | 2,898 | 2,782 | **2,701** |
| `anchor.js fields` — synthetic labels that are really record fields | 334 | 0 | 0 | **0** |
| stride-adjacent `DAT_`s that resolve to nothing | 12 | 5 | 5 | **3** |
| functions carrying a record stride | 692 | 303 | 115 | **115** |
| functions touching *only* unnamed globals | 545 | 478 | 471 | **397** |
| functions touching a *named* global | 902 | 968 | 974 | **878** |

The last column is a **later measurement**, not only a later typing: `symbols.json` grew
between the two, so re-measuring the third column's tree today gives 16,209 / 2,388 / 2,780
rather than 17,192 / 2,461 / 2,782. The two grids' own contribution, measured against that
same-day baseline, is **−901 `DAT_` occurrences and −79 `DAT_` names, of which 79 were the
grids' synthetic field labels and 7 were a folded base that resolved elsewhere once the
range was typed**. The last two rows count only *unnamed* functions, so naming two of them
in the same pass moves them out of both.

Seventy-two functions crossed from "unanchored" to "in a cluster" without anybody looking at
one of them, and 520 synthetic globals stopped existing. A step function that used to read

```c
if ((&DAT_0052f0b8)[g_movingUnit * 0x1a4] == '\x03') { ... }
(&DAT_0052f0bb)[g_movingUnit * 0x1a4] = (&DAT_0052f0bb)[g_movingUnit * 0x1a4] + -1;
```

now reads

```c
if (g_units[g_movingUnit].kind == 3) { ... }
g_units[g_movingUnit].y = g_units[g_movingUnit].y - 1;
```

**A grid indexed by byte offset needs no special shape.** The two 8-byte grids — `g_tiles`
(4,096 tiles, 64 × 64) and `g_battlefield` (6,400 cells, 80 × 80) — are not indexed like the
other six arrays. The game keeps a *pre-scaled* byte offset, `(y * 64 + x) * 8`, in a
variable and adds `±8` for a column and `±0x200` for a row, so there is no `i * stride` for
`anchor.js stride` to find and no obvious way to tell Ghidra "divide by eight". The move that
was expected to be needed — naming the eight plane bases as separate globals — turned out to
be unnecessary. **A plain `Tile[4096]` array handles both idioms**, because the decompiler
picks the reading that fits each site:

```c
g_tiles[local_1c * 0x40 + local_18].flags     /* the loader, which indexes by tile */
(&g_tiles[0].unit)[tileOffset]                /* everyone else, who carries the byte offset */
(&g_battlefield[0x50].terrain)[cellOffset]    /* ... and one row south */
```

The second form is the interesting one: it names the *plane* and leaves the byte offset
visible, which is exactly the game's own model. The third shows the payoff on neighbour
arithmetic — `[0x50]` is 80 cells, one row down, and used to be a bare `DAT_00544360`.
Nothing about the shape had to be invented; the only decision was to give the record its
true 8-byte size and let the offsets fall where they fall.

Two things this shape does for free. Both grids come out **100% named with no padding**,
where the six stride-indexed records are 23–92%; and one long-standing decompiler artefact
fixed itself — `Minimap_Click` read `(&DAT_0052ae10)[x + (y-0x19)*0x80]`, a base folded
0x180 bytes *inside* `g_tiles`, and once the range was typed Ghidra re-folded it onto
`g_minimapCounty`, where it belongs.

**The widths are measured, not assumed.** `ghidra_scripts/RecordProbe.java` walks every
instruction, folds each address that lands in a record array to an offset within the record,
and reports the p-code `LOAD`/`STORE` width used there. `docs/records.json` carries a field
only where the documented meaning and the observed width agree; everything else stays
undefined padding, which is why the six stride-indexed structs are 23–92% named rather than
100%. On the two grids the probe is unanimous: **441 references to `g_tiles` and 566 to
`g_battlefield`, spread over all sixteen planes, and every one of them one byte wide but
two** — and those two are `PUSH 0x5440e0`, the array's own address handed to the renderer,
not a read of a cell. That is what makes an all-`u8` layout a measurement rather than a
reading of the documents. The check the
retyping then passes is that **no widening cast straddles a named field anywhere in the
corpus** — if a field were typed one byte too narrow, some function would be reading across
its boundary, and none is.

**A struct that does not fit is evidence a record is wrong, and three did not.**

* `docs/battle.md` §2.1 says the figure's second target pair at `+0x28`/`+0x2A` is "filled
  from unit `+0x26`/`+0x28`". It is not. The only non-zero write to it in the whole binary
  takes the unit's `+0x16`/`+0x18` — the pair `docs/battle-ai.md` §7 calls the catapult aim
  point — and the three other writers only zero it. `records.json` names it the figure's own
  aim point on that evidence.
* `crates/l2-sim/src/runner.rs` calls figure `+0x144`/`+0x146` "where this figure is walking
  to … written by `Formation_SendFigure` and by nothing else". **No instruction in
  `Lords2.exe` references either offset**, and both fall inside the 150-pair path array at
  `+0x38`. `docs/battle.md` §2.1 and `docs/battle-ai.md` §7 agree with each other that the
  field is `tg x`/`tg y` at `+0x24`/`+0x26`, and the probe finds 41 sixteen-bit accesses on
  each. The documents are right and the Rust comment is wrong.
* `crates/l2-kingdom`'s `Industry` carries `capacity`, `efficiency` and `disabled_seasons` as
  `i32`. In the original `capacity` is **two** bytes at industry `+0x0E` — `+0x10` is the
  running total and there is no room — and the other two are single bytes at `+0x04` and
  `+0x06`. Nothing overflows, so this is a widening rather than a bug, but the record is not
  four `i32`s and reading it as one would misplace every field after the first.

**The two grids produced three more, and one of them is the biggest.**

* `docs/battle.md` §3 gives the battlefield `terrain` byte as `skr.md`'s "Lords2 id" column —
  1 open, 3 rocks, 4 hills, 6 **unused**, 7/8/9 bridge, 11 water, 12 woodland, 13. Retyped,
  `Battlefield_BuildRandom` (`0x0047AAA3`) is legible for the first time, and that list is
  the **output of a translation, and incomplete**. The builder maps each source raster byte
  to a runtime id — `0→1, 2→4, 4→0x14, 7→0x28, 8→0x29, 9→0x0B, 0x0A→0x0C, 0x0F→0x1E,
  0x10→7, 0x12→8, 0x14→9, 0x15→0x0D` — writes **6** (the "unused" id) for every source byte
  in `0x50..0x5F`, with the frame set to `source + 0x2C` and the impassable bit set, and
  passes anything else through **verbatim**. So `0x14`, `0x1E`, `0x28` and `0x29` are live
  terrain ids that appear nowhere in the document, and the pass that follows pairs `0x14`
  with the cell one row south and `0x28` with `0x29` in runs of up to four — they are
  multi-cell structures.
* `docs/battle.md` §3 calls cell `+4` **elevation**, and after the build it is. During
  `Battlefield_BuildCastle` (`0x0047C4BA`) it is not: the builder seeds it from a 256-entry
  2-byte table (`0x004D7D80`, or `0x004D7B80` for the other tile set) whose second byte is
  the passability flag, and then **re-dispatches on values 5…12 as structure codes**,
  consuming each one and replacing it with a real elevation 0…4 plus a `surface` and a
  `flags` assignment. `+4` is an escape-encoded field mid-build and a height only afterwards.
  Both builders were listed in §3 as "neither was traced"; both now read.
* `docs/formats/maps-layers.md` §5.3 lists the run-time-only bits of the `bank` byte as
  `0x01`, `0x20` and `0x80`. Bit `0x40` is missing and is not idle: it has nine clears and
  six tests, and in `Map_RenderIso`'s two half-row loops `bank & 0x40` is the sole condition
  for calling **`Map_DrawCountyFlag`**, exactly as `bank & 0x80` is for the building overlay.

**And a negative result about the method, which cost a wrong claim.** A census of the bit
masks applied to each plane in the corpus is a **lower bound only**. It said the tile `flags`
bit `0x08` — `maps-layers.md`'s **[I]** "rough terrain" — is never read anywhere, which would
have been a good finding and is false: `Move_BuildCostMap` copies the byte into a local first
and tests `(bVar1 & 0xC) == 0`, so both `0x04` and `0x08` are read there, as
`docs/armies.md` §2.2 already said. Never conclude "nothing reads this" from a pattern that
requires the field expression and the mask to be adjacent.

The same run found `g_battleMen` and `g_battleUnits` are **81** records, not the 80 their
`symbols.json` comments say: every sweep is `for (i = 1; i < 0x51; i++)`, so index 80 is
live, and nothing else in the binary claims the storage behind it.

### 7.6 Two tiers of name, and the file that keeps them apart

§7.3's two rates are the reason this section exists. The filters produce leads faster than
anyone can check them, and there are only two things to do with an unchecked lead: throw it
away, or write it down somewhere that is **not** `docs/symbols.json`. Throwing it away is how
correction C21 happened — a whole layer went unexamined because nothing recorded that it had
not been. Pouring it into `symbols.json` and marking it `inferred` is how C3 happened.
`symbols.json` is kept at a deliberately high verified ratio, and **that ratio is the only
thing that makes it worth reading**; a bulk pass produces far more plausible names than
checked ones, so admitting them would destroy the property that makes the file useful.

So there are two files:

| | `docs/symbols.json` | `docs/hypotheses.json` |
|---|---|---|
| what goes in | **[V]** a check that could have failed, recorded in the `comment` | **[I]** a plausible name and the reason to think it |
| extra fields | — | `basis`, `confidence`, `caveat`, `promotion` |
| applied to Ghidra | yes, by `ApplySymbols` | **no** |
| in `docs/symbols.md` | yes, regenerated | no |
| in the decompiled corpus | yes, the function reads by name | no, it stays `FUN_…` |
| what it costs to cite | nothing — it is a finding | **it may not be cited as a finding at all** |

The last two rows are the whole of it. `symbols.json` reaches the decompiled corpus, where a
wrong name becomes a fact by repetition — the exact mechanism behind C3. **Nothing in
`hypotheses.json` is applied to anything.**

**Promotion is a move, not a copy.** Run a check that could have failed, add the symbol with
that check in its comment, and delete the entry from `hypotheses.json` in the same commit. An
address that is verified must not also sit in the hypothesis file: a name in both is a name
whose tier nobody can read off. Nothing is promoted by having sat there a while, and a name
that cannot be given such a sentence has not been promoted — it has been relabelled. If the
check goes the other way, **delete** the entry; do not soften it and do not mark it
"partially confirmed", because a refuted hypothesis left on the page is worse than one never
written — the next reader cannot tell it from the ones nobody has looked at yet.

Every entry carries a **`basis`**: the observation the guess rests on — an `L2.eng` group and
index, a widget-table membership, a shipped filename, a record stride, a named caller. Not
the reasoning, the *source*, so that when a basis turns out to be unreliable everything
resting on it can be found in one grep. An entry whose basis is only *"it is called from near
something named"* is the shape C3 took and should be read as a question, not an answer.

**`confidence` is an enum, always one of exactly three words**, saying *which half of the
name a reader should distrust*, in §7.3's vocabulary:

| value | means | §7.3 rate |
|---|---|---|
| `subject` | a string, an asset or a stride pins **what it is about**, and the role word in the name may be wrong | 89% right |
| `role` | the **role** is pinned — it is in a widget table, it is the painter a dispatcher calls — and the subject is the guess | 58% right |
| `both` | **neither half has an independent anchor.** A cluster of these that touches no string and no shipped file is where C3 lives | — |

Nothing else is a legal value. The point of the enum is that it is *countable*: it can be
totted up against the two measured rates, so the file can say which two entries in five to
check first rather than merely that it is unsure.

**`caveat` holds the prose.** What is anchored, what is not, and what would refute it. A
caveat that names no way of being wrong is not a hypothesis, it is a wish.

Three agents created the file independently on the same day, and for a while `confidence`
held either form — 36 entries an enum, 29 a paragraph — which makes it unreadable by anything
mechanical. The 29 were read and classified and their prose moved verbatim into `caveat`;
four field entries that carried neither, because their evidence had been written once on the
last member of a quartet, were given both.

**Migrating them said something the enum could not.** Several of the prose entries turned out
to express no doubt about the *name* at all: `County.purse`, `County.fieldsReclaimable`, the
`armyFood` pair and the four `Realm` trade accumulators are all anchored on both halves, and
what is missing is a **save that exercises them** — every one is zero in every fixture, so no
reproduction can confirm or refute it. That is a promotion note, not a confidence value, and
it already lives in `basis` and `promotion`. Hence three words and not four: an entry waiting
for a witness is not a fourth kind of uncertain name, it is a certain-enough name with no
data behind it.

`hypotheses.json` also carries a **`corrections`** array: claims elsewhere that a pass
believes are wrong but did not rewrite, because the entry belongs to another subsystem or
another agent is appending to the same file mid-flight. Each carries its evidence and a
`notDoneBecause`. That is not a to-do list; it is the alternative to silently leaving a known
error in place or reaching into somebody else's file while they are working in it.

#### 7.6.1 What "a check that could have failed" means in practice

The bar is not "I read the function and it does that". It is: **name a consequence of the
guess that the evidence could have contradicted, and go and look.** In ascending order of
strength:

* **A dispatch table read out of the file.** The slot index is a fact. Eleven functions in
  `g_troopTickTable` wrote eleven sets of constants and every set matched the troop on that
  row of `docs/battle.md` §6.1 — a wrong table order scatters them, so the agreement is
  evidence and not a restatement.
* **Arithmetic that closes with nothing left over.** A figure's sprite sheet splits into walk,
  strike, stand and — for bowmen only — a draw block, and the bow animation's otherwise
  unexplained `+ 10` lands exactly in the gap the other three leave. There is one way to fit
  it.
* **Numbers from outside the binary.** Six animation functions pairing with six others is the
  shape of C3. What made it evidence was the *shipped art*: the `a3` sheets hold `8N + 13`
  frames for exactly the six N the `a3` handlers use, and `A3_horse.pl8` has 8 frames against
  `A2_horse.pl8`'s 48, matching `horseFrame = dirc` against `dirc * 6`. The binary cannot
  have arranged that.

#### 7.6.2 Constraint propagation, and its failure mode

Naming a subsystem one function at a time costs the same for the thousandth as for the first.
Naming it as a graph does not: **a guess is a set of predictions about its neighbours**, and
each prediction that holds makes the next one cheaper. If X is the fight-or-autocalc prompt,
its callers are battle initiation and its callees are the autocalc path; go and check.

The danger is the one C3 names, and propagation makes it *worse*, not better: a network of
mutually supporting wrong guesses feels more convincing the larger it grows, because every
new member is consistent with all the others. Two rules:

* **Never let a region float free of an anchor.** An anchor is something that cannot lie — a
  table read out of the file, a string the game displays, a saved-game value, a frame count
  in the shipped art, an invariant that closes. A cluster that is coherent and touches
  nothing external is **[I]** at best, and saying so is the deliverable.
* **Say where the network stayed coherent and unanchored.** The battle pass ended with
  exactly one: the bow-draw animation reads the *campaign* map's rotation and nothing else in
  the battle does. Two stories fit and no evidence separates them, so it is written down as
  unresolved rather than narrated into place. That paragraph is worth more than the fifty
  names around it, because it is the one place the next person should look first.

#### 7.6.3 The tools are grep with structure, not an authority

§4 records what `anchor.js`'s C-literal bug cost. The operational consequence belongs here:
**anything small and enumerated — state bytes, unit kinds, order ids, troop indices, screen
ids — is exactly what gets written as a character escape**, so a figure that came through a
tool must be confirmed against the corpus text or the bytes before it is load-bearing. The
corpus is the evidence; the tools are a convenience over it.

---

**Two additions to that file, made when a second subsystem started using it.**

* A **`promotion`** field beside `basis`: the check that would move the entry into
  `symbols.json`, *named in advance*. That is the whole point of it — a promotion
  criterion invented after the evidence arrives is not a criterion, it is a
  rationalisation, and this is the same discipline §4's *"a test that cannot fail is not
  a test"* asks of code. "Read it" is a perfectly good promotion criterion; "it will
  become clear" is not.
* A **`claims`** array, for the readings that are not about one address — a field's
  meaning, an off-by-one, a rule. Those are exactly the ones that previously had nowhere
  to live, so they went into prose and were re-derived later by someone who did not know
  they had been derived.

And one rule the file did not state and now does: **an address verified in `symbols.json`
must not also appear here.** Two subsystems reached `g_screenIdSaved` from opposite
directions within a day, one filing it as a hypothesis and one as verified. The verified
entry wins and the hypothesis is deleted, which is what "promotion is a deletion plus an
addition" already meant — but nothing had said what to do when the two happen
concurrently.

## 7.7 Constraint propagation, and where it stops being cheap

Naming functions one at a time and checking each independently is the safe technique and the
slow one. The alternative is to treat a guess as **a set of predictions about its
neighbours** — if X steps a unit, its caller is the mover and its callees are the cost map
and the path array — and to test the predictions rather than the guess. Survivors anchor
their neighbours, and the cost per function falls as the picture tightens.

It works, and it has a specific failure mode: **coherence is not correctness.** C3 is a
self-consistent and entirely false model that assembled itself out of plausible parts, and a
propagating network can lock one in that feels *more* convincing the larger it gets. The
discipline that makes the technique safe is to pin the network, repeatedly, to things that
cannot lie:

* **`L2.eng` strings the game displays.** Index 0 of a group is a descriptive label, so a
  function that draws group 281 is drawing *"Cannot siege castle"*. This is the highest-yield
  anchor in the binary and it is nearly free — §7.2.
* **Arithmetic that closes.** A 4×4 scan whose row stride is `512 − 4×8`; a `memset` of
  `0x2000` over a 64×64 `i16` grid; a serialiser that emits `1+4+1+2+2` bytes beside a length
  table that says 10. Each of those could have come out wrong.
* **The save fixtures.** A number the code computes, found in a file the game wrote.

**A cluster that touches none of the three is where C3 lives.** Flag it and leave it in
`hypotheses.json`; do not commit it to `symbols.json` because the rest of the cluster agrees
with it.

Measured on one campaign-map pass: **21 role predictions made, 15 held, 6 refuted.** Three of
the six refutations were the most valuable results of the pass, because a refuted prediction
in a propagating network invalidates its neighbours too and stops the error spreading. The
subject filter (§7) was right far more often than the role guess — as calibrated — so treat
"this function is about sieges" as nearly free and "this function lifts a siege" as a claim
that still needs its own check.


## 8. What "done" means

The roadmap has eight phases and they have been advanced roughly in parallel, which is why
"all phases complete" keeps not being true: every phase has an open-ended tail, and there is
always more of the binary to name — <!--fig:functions-->676<!--/fig--> of <!--fig:binary-functions-->2,452<!--/fig--> functions so far, about <!--fig:functions-pct-->28<!--/fig-->%.

Naming the remaining 90% is **not** the goal and mostly never will be: most of it is CRT,
allocator, string and DirectDraw glue. The goal is a *playable, moddable engine*, and the
honest measure of progress is a vertical slice that runs end to end, not a percentage of
functions understood.

The current gap is integration rather than knowledge. The battle simulation runs and now
draws itself; the kingdom economy computes; the netcode syncs; mods load. **None of them are
joined up into a game you can sit down and play.** That is the work that matters next, and
it is also the only thing that will reveal which of the remaining unknowns are load-bearing
and which are trivia.
