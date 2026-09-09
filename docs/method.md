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

| | before | county/unit/realm/lord | + the two battle arrays |
|---|---:|---:|---:|
| `DAT_` occurrences in the corpus | 24,608 | 19,661 | **17,192** |
| distinct `DAT_` names | 2,977 | 2,575 | **2,461** |
| distinct globals the corpus sees | 3,302 | 2,898 | **2,782** |
| `anchor.js fields` — synthetic labels that are really record fields | 334 | 0 | **0** |
| stride-adjacent `DAT_`s that resolve to nothing | 12 | 5 | **5** |
| functions carrying a record stride | 692 | 303 | **115** |
| functions touching *only* unnamed globals | 545 | 478 | **471** |
| functions touching a *named* global | 902 | 968 | **974** |

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

**The widths are measured, not assumed.** `ghidra_scripts/RecordProbe.java` walks every
instruction, folds each address that lands in a record array to an offset within the record,
and reports the p-code `LOAD`/`STORE` width used there. `docs/records.json` carries a field
only where the documented meaning and the observed width agree; everything else stays
undefined padding, which is why the structs are 23–86% named rather than 100%. The check the
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

The same run found `g_battleMen` and `g_battleUnits` are **81** records, not the 80 their
`symbols.json` comments say: every sweep is `for (i = 1; i < 0x51; i++)`, so index 80 is
live, and nothing else in the binary claims the storage behind it.

### 7.6 The bulk tier: `docs/hypotheses.json`

§7.3's two rates are the reason this file exists. The filters produce leads faster than
anyone can check them, and there are only two things to do with an unchecked lead: throw it
away, or write it down somewhere that is **not** `docs/symbols.json`. Throwing it away is how
correction C21 happened — a whole layer went unexamined because nothing recorded that it had
not been. Writing it into `symbols.json` is how C3 happened.

So there are two tiers, and blurring them is the failure both corrections describe:

| | `docs/symbols.json` | `docs/hypotheses.json` |
|---|---|---|
| what it holds | a name **plus the check that could have refuted it and did not** | a name plus the mechanical observation it rests on |
| applied to Ghidra | yes, by `ApplySymbols` | **no** |
| read by the decompiler output | yes | no |
| the ratio that matters | verified stays high on purpose | there is no ratio; it is all guesses |

Every hypothesis entry carries a `basis` — the observation, stated so that someone else can
disagree with it — and a `confidence` in §7.3's own vocabulary:

* **`subject`** — an `L2.eng` group, a `.pl8` filename or a record stride pins *what it is
  about*; the role word in the name is the part that may be wrong. This was 89% right.
* **`role`** — the role is pinned (it is in a widget table, it is the painter a dispatcher
  calls); the subject is the guess. This was 58% right.
* **`both`** — neither half has an independent anchor. **A cluster of these that touches no
  string and no shipped file is where C3 lives.** Read it before using it, and prefer
  deleting it to promoting it.

Promotion is a deletion plus an addition: remove the entry here, add it to `symbols.json`,
and put the check that promoted it into its `comment`. A name that cannot be given such a
sentence has not been promoted — it has been relabelled.

## 8. What "done" means

The roadmap has eight phases and they have been advanced roughly in parallel, which is why
"all phases complete" keeps not being true: every phase has an open-ended tail, and there is
always more of the binary to name — 465 of 2,452 functions so far, about 19%.

Naming the remaining 90% is **not** the goal and mostly never will be: most of it is CRT,
allocator, string and DirectDraw glue. The goal is a *playable, moddable engine*, and the
honest measure of progress is a vertical slice that runs end to end, not a percentage of
functions understood.

The current gap is integration rather than knowledge. The battle simulation runs and now
draws itself; the kingdom economy computes; the netcode syncs; mods load. **None of them are
joined up into a game you can sit down and play.** That is the work that matters next, and
it is also the only thing that will reveal which of the remaining unknowns are load-bearing
and which are trivia.
