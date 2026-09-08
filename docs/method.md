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

## 7. What "done" means

The roadmap has eight phases and they have been advanced roughly in parallel, which is why
"all phases complete" keeps not being true: every phase has an open-ended tail, and there is
always more of the binary to name — 230 of 2,452 functions so far, about 10%.

Naming the remaining 90% is **not** the goal and mostly never will be: most of it is CRT,
allocator, string and DirectDraw glue. The goal is a *playable, moddable engine*, and the
honest measure of progress is a vertical slice that runs end to end, not a percentage of
functions understood.

The current gap is integration rather than knowledge. The battle simulation runs and now
draws itself; the kingdom economy computes; the netcode syncs; mods load. **None of them are
joined up into a game you can sit down and play.** That is the work that matters next, and
it is also the only thing that will reveal which of the remaining unknowns are load-bearing
and which are trivia.
