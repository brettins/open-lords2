# Working with parallel agents

This project is far larger than one context window. Agents are how it scales, and file
ownership is how they stay out of each other's way.

## Ownership

| Resource | Owner |
|----------|-------|
| `docs/status.html` | lead session only |
| `crates/l2-formats/` | lead session only — agents report logic to integrate, they don't edit it |
| `CLAUDE.md`, `docs/decisions.md`, `docs/symbols.md` | lead session only |
| A format doc under `docs/formats/` | whichever agent is working that format |
| A tool subdirectory under `tools/` | whichever agent created it |
| `docs/work.json` | lead session only, on branch `pm/ledger` — see *The work ledger* below |

Give every agent its own output files. Two agents editing one document will clobber each
other, and neither will notice.

## The work ledger: what is in flight lives in a file, not in a conversation

`docs/plan.md`'s in-flight lists read as current long after they were not. Two "held for
cause" verdicts were wrong by the time anybody acted on them, and a machine restart needed an
emergency dump of state that existed only in the lead's session. The player put the fix in
25: one sentence: *"your context window is not a good project management understanding — you
26: should be an interface for whatever project management setup you're running."

That setup is **`docs/work.json`**: one row per piece of live work. **The failure it exists
to prevent is the one `plan.md` suffered — a status that looks current and is not.**

### The protocol, which binds the lead

1. **Write or update the row before dispatching an agent, and before replying to an agent's
   report.** A row written afterwards is a memory. Written before, it is what the next session
   starts from if this one ends mid-sentence.
2. **Rows store intent only**: what, why, where it came from, what it depends on, the next
   step. **Anything git can answer** — merged or not, commits ahead of `main`, `HANDOFF.md`
   present, the last commit — **is derived** by `node tools/pm/work.js --status` on every run
   and is never typed in. `--check` refuses a row carrying such a field, by name. A stored
   "not merged" is a claim with no timestamp; a derived one cannot go stale.
3. **Merged rows leave the file**, and every `depends_on` that named them goes with them.
   `git log` is the record of what landed. A ledger that keeps finished rows becomes a second,
   drifting copy of it.
4. **The lead is the only writer of `docs/work.json`, on branch `pm/ledger`.** Agents report;
   they do not edit it. The file merges by `id` through the keyed driver, keeping its row
   46: order (the merge queue is read top to bottom) and its one-row-per-line shape, so a merge
   between `pm/ledger` and `main` cannot misalign rows. One writer is what keeps it from
   needing to.

### What `--check` enforces, and where each half runs

* **Schema**, which needs only the file, runs everywhere, and runs on every push through
  `crates/l2-testkit/tests/work_ledger/main.rs`: every row has every field and no other; state
  and track are declared at the top; ids are unique; `depends_on` names rows that exist and
  has no cycles; an `in-flight` or `queued-merge` row names a branch.
* **Agreement with git**, which needs the clone the work happens in: every branch a row
  names exists; no `in-flight` or `queued-merge` row names a branch already merged into
  `main`; no unmerged `worktree-agent-*` or `wip/*` branch with commits ahead of `main` goes
no `queued-merge` branch carries `HANDOFF.md`. A fresh clone or a CI runner
  60: has no agent branches, so this half **prints `SKIP` with the reason and the number of rows
  it did not compare**

`--status` is the derived view as text. `--html <path>` writes **the player's page**: the
original game's features graded done / partial / missing / not assessed from
**`docs/features.json`**, then what is in progress, what merges next, what waits on him, and
66: the backlog and the numbers folded away — one line per item, a ledger row's title and
nothing else. `--html-detail <path>` writes the full page for agents. Both are generated,
68: **never committed**, with the time and the `main` SHA on them so their own staleness is
visible. The player's first page printed every row's prose and had no feature list; he
called it *"blabby"*.

**`docs/features.json` is a claim, so `--check` holds it to evidence.** A `done` feature must
73: cite a correction, a path, or an `arms:`/`audio:` inventory entry, and the full check proves
each exists on `main`; every ledger row a feature names must be in the ledger — **when a row
merges and leaves, the feature citing it goes red and must be re-graded**. A missing or
partial feature citing no row is reported, not failed: it is work nobody has written down.

**A view that names `main X` computes everything from `main X`.** The first version read the
systems inventories out of whatever checkout the tool sat in while its header said
`main 76a0437`. Run from an agent's worktree, it quoted that worktree's differential (251 of
279) and census (399) under main's name when main held 258 and 412. It also stamped the ledger
with the tool's own HEAD. So every figure is read with `git show <ref>:<path>` (`--ref`,
83: default `main`), and the ledger line names **the ledger file's** checkout, branch and last
commit, or says plainly that no commit holds it. The lead caught it by reading the page, which
is the only reason this paragraph exists; `work_ledger.rs` now holds both, in a scratch
repository whose working tree disagrees with its `main` on every inventory.

### Limits, stated, not discovered

* **"Merged" means the branch tip is reachable from `main` and is not on `main`'s own
  first-parent line.** The second clause is what tells a merged branch from one cut a minute
  ago: both are zero commits ahead. Every merge here is a merge commit, which the rule reads
  correctly. A **fast-forward** would read as "no commits yet", and a **squash or
  cherry-pick** as unmerged. If the integrator ever merges that way, the rule changes with it.
* **An unmerged agent branch is flagged whether it is live or long dead.** The check cannot
96: tell a stopped agent from an abandoned one without a row.
  Deleting a dead branch is the fix, and it costs one command.
* **The facts are this clone's.**
  exist as far as `--status` knows.

## Ghidra

**Only one process may hold a Ghidra project at a time.** Concurrent agents must use
separate project directories under `E:\dev\ghidra-projects` — e.g. `lords2` for the game
binary and `mapl2` for the shipped map editor. Say so explicitly in the agent's brief;
106: this is not obvious and the failure mode is a lock error mid-run.

Copy `ghidra_scripts/DecompileFunc.java` into a per-agent script directory
having several agents edit the shared one.

## Briefing an agent

Include, every time:

- The paths and how to run things (or point at `docs/environment.md`)
- **What is already established**, so they don't re-derive it
- **What was already tried and failed**, and why
- The validation standard — what would make their conclusion trustworthy
- Which files they own and which they must not touch
- That they must not commit, must not copy GPL code, and must not modify the game install

Ask for reports that separate **verified** from **inferred**, and that state what remains
unknown. An agent reporting "done" without saying what it could not establish is a report
you cannot act on.

**Tell them how to test.** A feature agent runs `powershell -File tools/run/changed.ps1`,
which diffs against `main`, maps the touched files to crates and runs `cargo test -p` for
each. The **integrator** runs the full workspace suite, once, per merge — not every agent
on every edit.

## The oracle check: a second agent reads the delivery against the binary

A green suite proves the code does what its tests say, not what the original does. The
AI turn ran inside End Turn from the first playable build (c026a579, 2026-09-07: *"one
step per realm per tick"*, a driver invented to make the spine turn over) until the
player noticed on 2026-09-14; `Turn_Tick`'s phase-4 arm had always said every frame.
Nothing between those dates re-read the arm, because every later agent took the
existing order as established.

So every delivery that touches behaviour gets a **read-only oracle check** before it
merges: the `oracle-check` agent (`.claude/agents/oracle-check.md`, Opus, 25-call
budget, read-only tools), given the branch name and nothing else.
For each behaviour in the diff it: names the function and address the code cites (rule
5), reads that function in `tools/oracle/decomp/` (dossier.js first), and answers three
questions: *same order of operations, same guards, same constants?* A behaviour with no
address cited is itself a finding. The report is a list of mismatches with the line of
decompilation beside the line of Rust, and *nothing else*; "matches" lines are not
written. The lead fixes or files each mismatch as a ledger row before the merge. Doc
chores, test fixtures and pure moves skip the check.

## The prose pass

The player's rule, 2026-09-13: the plainest listing of facts, and losing a clause is the
accepted price. `tools/review/prose.js` holds the phrase list (`--phrases`) and lists the
lines that carry one, per file. `tools/review/prose-llm.js <file>` sends that listing to
Gemini Flash (`GEMINI_API_KEY`, no thinking) with a delete-only prompt and applies the
reply through `prose.js --apply`; `--top N` takes the N worst files. `.claude/hooks/
trim-prose.js` runs it on every doc an agent edits (Markdown only since 2026-09-14: on
Rust it cut clauses out of cited facts). Rust comments get `.claude/hooks/comment-lint.js`,
which hands the lines back for the agent to restate in the shape below. Cost measured: about 2 s and 700 tokens per file by API, against 55-80k
tokens per file when an agent did the same job; never give this to an agent. After a pass
the lead runs `corrections.js --relock`, `cargo check --workspace --tests`, counts the
`fig:` markers per doc, and commits.

### The comment shape

The shape, and every line below already stands in the crates:

```rust
/// `Turn_End` (`0x0043AC23`) writes 999 when the person presses the button.

/// `Ai_ManageCountyFarms` (`0x0049DD01`) copies the lord's style byte into
/// county `+0x1FE` and dispatches on it.

/// County `+0x1FD` weapon type 0..5 → the `Merchant_Trade` good id:
/// `0 → 12, 1 → 11, 2 → 13, 3 → 9, 4 → 10, else → 14`.

/// `Ration_Apply` (`0x0044DF5F`) has no store `-=` anywhere in it.

// `Msg_Dismiss` (`00470000.c:2444`): a game-over letter's dismissal is
// `Campaign_EnterConquest(); g_screenId = 0x1C`.
```

A doc block is those lines in this order: the function, its address and the decompilation
line; what it does, with the numbers; then one line per claim that is inferred or a
departure, `[I]` or `[D]` with the reason. A test's doc names the claim and the line whose
deletion turns it red. The past tense belongs in `docs/decisions.md`, with a C-number.

## File size

The player's cap, 2026-09-13: a test file stays under 300 lines, a source under 500, and a
new file under 300. `.claude/hooks/file-size.js` tells an agent when a file it edited is
over the cap. The split is a script, never hand work: `tools/review/split-llm.js <file>`
(Flash plans line ranges from the outline, the script moves every line; a `tests/x.rs`
becomes `tests/x/main.rs`, anything else `x/mod.rs`) then `tools/review/widen.js <crate>
crates/<crate>` until `cargo check --workspace --tests` is clean. Widen reads every seam
class rustc reports and escalates `pub(super)` to `pub(crate)` for a cousin; what it cannot
read is a file's own doing (a cut inside a const, a self-reading test) and goes back to the
lead. 215 files went through it in one evening at about 3 s each.

## What agents are good and bad at here

**Good:** bounded investigations with a clear validation test (crack a format, find a
function, validate a hypothesis across a corpus). High-volume mechanical work where a
verifier exists — decoder loops, bindings, batch extraction with round-trip checks.

**Bad:** open-ended interpretation of decompiler output with no way to check the answer.
That is where a confident, wrong result costs more than it saves. A cheap model is safe
exactly where a verifier exists, and dangerous everywhere else.

## Commit *your own* paths, and know that `git add` is not enough

`git add -A` sweeps up whatever every other agent has written so far, which means
committing half-finished work under an unrelated message — and, once, fifteen rendered PNGs
of game data straight past the first rule of the project.

So stage explicit paths. **But staging explicitly does not make the commit safe**, and this
cost us a second time: `git commit` commits **the whole index**, not the paths you happened
to add. If another agent has already staged something — a file rename, say — your commit
takes it too, however careful your `git add` was. One agent swept another's staged renames
this way while following the explicit-path rule to the letter.

The form that holds names the paths on the **commit**:

```bash
git commit -F msg.txt -- crates/l2-mods docs/modding.md   # yes: only these, whatever the index holds
git add crates/ && git commit -F msg.txt                  # no: commits the whole index
git add -A                                                 # never
```

`git commit -- <paths>` bypasses the index for those paths and commits exactly what you
name. Check `git status --short` first and confirm every path is yours; when several agents
209: are live, that check is necessary and not sufficient, and the `--` form is what closes the
gap.

**One catch, and it bites on the first commit of anything new.** `git commit -- <paths>`
213: only accepts paths git already knows, so a brand-new file fails with
`pathspec … did not match any file(s) known to git`. A new file must be `git add`-ed first,
and *then* committed with the `--` form:

```bash
git add tools/oracle/xref.js
git commit -F msg.txt -- tools/oracle/xref.js    # add makes it known, -- keeps it alone
```

The `git add` is safe here because it names one path; it is the bare `git commit` afterwards
223: that would sweep the index, and the `--` prevents

**And a second catch, sharper than the first: `--` is wrong for removals.**
`git commit -- <paths>` commits the **working tree** at those paths and ignores the index.
That is exactly what makes it safe for edits — and it silently *reverses* a deletion you
staged with `git rm --cached`, because the file is still on disk. Untracking 1,891 build
229: artefacts this way put all 1,891 straight back, and the commit looked like it had worked.

So the honest rule is not "always use `--`". It is:

| what you are doing | form |
|---|---|
| adding or editing files, agents live | `git commit -F msg -- <paths>` |
| a **removal** (`git rm --cached`) | stage it, check `git status`, plain `git commit` |
| a brand-new file | `git add <path>` first, then the `--` form |

A removal cannot be isolated by `--`, so it has to be done when the index is otherwise
empty — which means **when no other agent is mid-edit.** If you need to untrack something
while agents are running, wait.

## Clean up processes you start

**Any agent that launches a process must terminate it before reporting.** The game in
particular takes over the entire screen, so leaving it running blocks the user from
seeing their own desktop, and they have no way to tell whether the agent is still using
it or abandoned it.

```powershell
Stop-Process -Name Lords2,l2-view,dxwnd -Force -ErrorAction SilentlyContinue
```

Run that at the end of the task whether it succeeded or not. Put the requirement in the
brief — an agent told it may run the game will not infer that it should also close it.

Prefer short focused sessions with the game over keeping it open across a long
investigation, and if a task needs it open for a long stretch, say so in the
report so the cost is visible.

261: ## Never drive the real mouse or keyboard to test our engine

**An agent testing our engine does not touch the OS input queue.** Synthetic input moves
the cursor and steals focus on a machine somebody is sitting at — this was found the way
these things are always found, by the person whose mouse jumped.

**everything our engine does is reachable as a
value**. `Event::Click { x, y }`, `Event::Pointer`, `Event::KeyDown`, handed to
`Machine::handle` with a `Ctx`; `crates/l2-game/tests/screens_*.rs` and `tests/machine.rs`
are both written that way already. For a visual result, render into a `Canvas` and inspect
the pixels — there is an ignored test that dumps PNGs on demand. No window, no focus, no
cursor.

`tools/input.ps1` stays legitimate for driving the **original** game as an oracle, which is
what it was written for. Even there, reach for it last: `tools/screen.ps1`'s `printwindow`
method captures a background window **without** focus, so most oracle work never needs the
desktop at all. An agent that believes it needs synthetic input against `Lords2.exe` should
**ask first**, because the cost lands on whoever is at the machine and they cannot tell an
agent's mouse from a fault.

**The general principle, which is the part worth carrying elsewhere: prefer the mechanism
that needs nothing of the world.** An in-process event cannot be disturbed by focus, a
screen lock, a resolution change or a person moving the mouse. It is also faster,
deterministic, and runnable in CI — which the OS route can never be. That is the same
reasoning behind reading a struct definition
behind a check that runs on every push, not a rule an agent is asked to remember:
**the mechanism with fewer dependencies on the world is usually also the more accurate
288: one**, and where the two pull apart it is worth noticing why.

## Concurrent agents: unique scratch paths, and count before and after

Two agents picked the same scratchpad filename on the same day, and one spliced the other's
half-written symbol list into `docs/symbols.json` by mistake. It was caught, reverted and
redone. What caught it was **a count check** — read the number of entries before the edit and
after it, and confirm the difference is the number you meant to add.

Two rules follow, and they are cheap:

* **Every agent uses scratch filenames unique to itself.** Put the agent's own id in the
  path. A shared temp directory with a predictable name — `out.json`, `syms.json`,
  `tmp.txt` — is a collision waiting for the day two agents run at once, and that day is
  now normal here, not rare.
* **Count before and after any edit to a shared JSON file**, and say both numbers in the
  report. `docs/symbols.json`, `docs/hypotheses.json` and `docs/records.json` are all
  appended to by several agents at once. A count that moves by the wrong amount is the only
  cheap signal that something arrived that you did not write, and it is how this was found.

### Correction numbers: take the next one, and say which you took

`docs/decisions.md` runs C1, C2, C3 … and an agent that finds something worth recording
takes the next free number. With several agents running, they all read the same log and all
reach for the same number: **this happened five times in one day** — three agents on C22,
and two each on C25, C28, C30 and C31.

**This is now checked mechanically.** `node tools/decisions/corrections.js --check` runs in
316: CI and fails on a duplicate number, naming both headings and the next free one; it also
fails on a citation of a correction that does not exist, which is the other half of the same
318: problem and the one that went unnoticed for weeks. Run it before you finish and you will not
hand the integrator a collision.

321: It is still not worth serialising the log, and the integrator can renumber safely. What
makes that cheap, not archaeological, is one line:

> **Say in your report which correction number you took**, and grep the tree for
> cross-references to it before you finish — `symbols.json` comments, Rust doc comments and
> other documents all cite corrections by number.

The integrator then renumbers the later arrival deterministically
collision by reading two entries with the same heading. Renumbering is a rewrite of the
heading **plus every citation**; a correction whose number moved and whose citations did not
is worse than the collision, because the reference now silently points at somebody else's
correction. That is exactly what happened to the C22/C23 pair, and `crates/l2-kingdom`
carried a citation to a correction that had never been written at all for weeks — see C26.

The integrator repeats the check at merge time — union-merging by address, then scanning for
an address or a name that appears twice — but that is a second line of defence. The agent
doing the edit is the one who can still tell what it meant to write.

**The check verifies that a citation resolves, not that it resolves to the *right*
correction** — and those are very different. A renumber done by search-and-replace drags
unrelated citations along: they all resolve, the lint passes, and each one now silently
points at somebody else's correction. **That happened four times in one session.** Once for
real, when renaming a correction to C36 took `kingdom.md`'s reference to the secession tie
with it; then three more times while renumbering colliding corrections, stranding four
citations at C36 and later six at C38 — with the lint reporting *"276 citations, all
resolve"* each time. Every one was found by a person reading.

So there is a third rule, and it is a **lockfile**: `tools/decisions/citations.lock` records,
for every citation, a hash of the words around it with the C-numbers blanked. A dragged
citation keeps its words and changes its number, which is exactly what that compares. Adding
or rewording a citation makes the lockfile stale and asks you to regenerate it; a *drag*
fails loudly and names the line to put back.

    node tools/decisions/corrections.js --relock

**Why a lockfile and not the obvious alternative, which is worth more than the mechanism.**
357: The first proposal was to make citations self-describing — write `C34 (secession-tie)`, so a
dragged citation carries its old slug against its new number and fails immediately. It was
refuted by measuring, not by arguing:

* **80 of 261 citations are shapes that would not survive it.** Lists especially:
  *"C10, C12, C17 and C20 are four instances of one mechanism"* becomes unreadable when every
  number carries a parenthetical, and these documents are written to be read.
* **Half the slugs derived from headings say nothing** — `files-fail`, `names-doing`,
  `reading-five`.
366: * And the detail that ended it: **two slugs embed a different correction's number.** C16's
  heading opens by referring to C14 and C20's to C12, so the mechanical slugs come out
  `c14-second` and `c12-second`. **A disambiguation scheme derived from the thing being
  disambiguated can inherit its ambiguity**, and a slug that names the wrong correction is
  worse than no slug at all.

The lockfile costs 10 KB, has no migration, changes nothing about how a citation is written,
373: and flagged the real historical failure with zero false positives across a merge that moved
seven citations and drifted every line number in the tree. That is the general lesson too:
**when two mechanisms are proposed, measure them against a failure that happened**
.

**An honest note about the paragraph above, because the record is worth more than the rule
looking effective.** The protocol in this section was written after four collisions and then
did not prevent the fifth —: the branch that collided had
been cut *before* the protocol landed, so the agent never read it. So
either way about whether writing it down works, and there is now no need to find out.
Documented process depends on an agent having read the document, which a long-running branch
by construction may not have; a check in CI does not. That is the general lesson, and it is
worth applying to the next process rule this file gains: **prefer the version a machine
enforces over the version an agent is asked to remember**, and if you write the second, plan
to replace it with the first.

## Ablate the line, or you have not tested anything

**This is the practice that would have caught most of what follows, and it takes thirty
seconds.** It is at the top of the testing sections for that reason: the individual traps below
are worth reading, but this is the one habit that finds them without knowing which one you are
in.

> **Delete the exact line the assertion claims to be about, and watch the test go red.**
>
> A test written against a passing tree has never been observed failing, and until it has,
> *"it passes"* is a statement about the tree and not about the test.

Not the code near it. Not a plausible neighbour. The line the assertion names.

### The case that made it a rule

The title screen carries a build stamp — short commit and date — because a player spent an
evening reporting already-fixed defects against a binary four merges old. A test was written to
assert it is on screen: count the non-background pixels in the band where the stamp is drawn,
and require enough of them.

It passed.

**It also passed with the line that draws the stamp deleted.** The title page carries a
413: full-screen `gateway.pl8`, so no pixel down there is background, and the test had been measuring
the artwork the whole time. Nothing about it looked wrong. The threshold had been chosen by
looking at what the passing case produced — which is exactly how a test comes to describe the
status quo, not the claim, and it is a very easy thing to do while being careful.

Thirty seconds of ablation found it. Nothing else would have, until the day the draw call was
419: lost in a merge and the stamp quietly stopped appearing —
stamp exists to prevent, so the test would have failed at the moment it mattered.

### Idempotence is a stronger assertion than effect

The replacement is worth recording as a technique, because it applies well beyond this screen:

**Where an operation is idempotent, assert idempotence, not an effect.**

Draw the page. Copy it. Draw the stamp **again** onto the copy. Require the two canvases to be
429: *identical*. Text is an opaque blit, so a second draw over itself changes nothing — but only if
430: it was there the first time. Delete the draw call and the second draw *adds* the stamp: 1,234
pixels differ.

That is exact. No threshold to tune, no knowledge of what else is on the page, and nothing to
re-tune when the artwork changes. Compare it with the pixel count, which needed a number chosen
by observation and was wrong about what it was observing.

The same shape is available more often than it looks: re-running an idempotent import, re-sorting
a sorted list, re-applying a migration, re-normalising a normalised file. Each turns *"did this
happen?"* — a question about effects, which needs a threshold — into *"is this already done?"*,
which is an equality.

### When ablation is not available

Some assertions have no single line to delete: a property over generated input, or a check whose
subject is a whole file. Then the substitute is to **make the check fail deliberately once**, by
corrupting its input, and read the message it produces. A check whose failure has never been
447: read is a check whose message is untested, and the message is most of the value — this project's
own convention is *fail with the fix in the message*, which is unverifiable until somebody has
seen one fail.

### How to ablate wrongly

The practice above is cheap and it is **not automatic**. Five ways it goes wrong, all of them
454: observed here, and the first is the one that limits the whole discipline.

**One: compute the probe from the constant you are ablating.** A test asserted that the pasture
cattle land at the original's `(+4, −4)` offset. It found its probe pixel by *applying that
458: offset* — so deleting the constant moved the probe with it, and the test stayed green. It was
asserting that the code agrees with itself, which it always will.

> **Ablation works, and ablating a constant while computing your probe from that same constant
> tests nothing at all.**

This is the one entry that limits a practice the rest of this document recommends without
qualification: **pin the literal from the oracle.**
The probe position is now a number read out of the decompilation, and no expression in the test
mentions the constant under test. Anyone who adopts the ablation habit will meet this case, and
it is not visible from the inside — the test is four lines long, it reads correctly, and it
passes.

**Two: ablate something the check deliberately excuses.** The encode/decode check carries a
`not-encoded:` marker for fields outside the codec. Testing it by renaming a field that carried
one produced a green run, which for a minute read as *the check is broken*. It was the check
474: working. An accidental **fail-to-fail** is indistinguishable from a broken instrument, and the
defence is to ablate something the check *claims*, not the nearest thing to hand.

**Three: insert a probe between a doc comment and its field.** The second attempt at that same
test added a field immediately below an existing doc comment — which, in Rust's model and in the
scanner's, attaches that comment to the new field. The probe silently inherited the excuse and
the check went on reporting the *old* field. Ablation by insertion moves whatever the insertion
point owned; ablate by deletion where you can.

**Four: read the count
failing crate, so an ablation that should turn three tests red reports **one**. Use
`--no-fail-fast`, and read *which* tests went red, not how many. The identity of the
failing test is the finding; the count is not.

**Five: ablate the draw and forget the viewport.** The build stamp's test has now failed three
times in three different ways, and it is the same feature each time. First it passed with the
draw deleted, because it counted pixels on a page with full-screen artwork behind it. Then
491: idempotence fixed that — and the stamp shipped **half off the bottom of the screen**, because
identity of two canvases proves the text was *painted* and says nothing about whether the place
it was painted is *presented*. The third version asserts every pixel the stamp writes falls
inside the visible canvas, and is ablated by moving it ten pixels down.

> **"Is it drawn" and "can it be seen" are different claims, and only one of them is what
> anybody wanted.**

Each of those three checks was true of what it measured. That is the pattern this whole document
keeps circling, and one small feature has now demonstrated it three times: **the question is never
whether the assertion holds, it is whether the assertion is the claim.**

## A test that drives the picture from the wrong field passes for ever

The village screen had **eleven tests and not one asked what the county's own record
said.** Every one drove the picture from the county's *labour* fields, which the scenario
importer fills. Not one drove it from the *industry* fields, which it did not — four
24-byte records per county were skipped wholesale, so `has_resource` and all four enable
switches came from `County::new()`'s defaults, and **every map toggle sat in the opposite
position to the one the player saw.** Eleven passing tests, and a struct field that no
importer had ever written.

513: That is C30 from the drawing side, not the save side, and the pair states the rule
better than either alone: **a field is only tested if something a test reads was written by
something the game runs.** A test that populates the state it then asserts on is checking
its own fixture. C30's four fields were absent from the *encoding*; these were absent from
517: the *import*; in both cases the suite was green and the field was fiction.

Two practical consequences:

* **When you add a field to a record, ask what writes it in a real game** — an importer, a
  season pass, a click — and make at least one test travel that road. If nothing writes it
  yet, that is worth knowing and worth saying at the field.
* **Prefer a fixture the game produced to one a test built.** `l2_testkit`'s gated
  fixtures exist for this; a hand-built `Kingdom` carries whatever `new()` gives it, and
  `new()` agrees with every wrong reading equally.

## A correct explanation sitting directly above the omission it describes

This is a new one. **The comment was
right. It was load-bearing. It was in the file. And it did not cause the work to happen.**

`crate::field`'s module docs had already worked out why the original's estimate round runs
twice — and said so, in a paragraph that reasons carefully and reaches the correct answer:

> *"That looked like a fixpoint and it is not one: reading the five estimate bodies, no ceiling
> depends on the current assignment … What the second round is for is the `Herd_UpdateCrowding`
538: > in the middle, which does move an estimate's input, **and the panel forecasts, which the
> estimates fill from whatever the allocator last decided.**"*

Twenty lines below it, `grain_labour_estimate` ports the search loop of
542: `Grain_LabourEstimate` and stops. **The four things the tail writes — the sowing, growth and
543: harvest forecasts and the signed change the sidebar draws — were carried by nothing**, and a
player found the hole by looking at the screen: *"Sidebar doesn't show grain being planted as
a negative number."*

> **Knowing why a pass exists is not the same as carrying what it writes.**

It belongs beside *a comment that defers work to a caller must name the caller* and *a
document that promises "until X" keeps promising it long after X* — three shapes of the same
551: thing, and the reason they are worth grouping is that **none of them is a missing comment.**
Every one is prose that is accurate, that a reviewer would nod at, and that describes work
which then did not get done. The usual defence on this project — write it down, cite it,
cross-reference it — is the thing that already happened.

**What would have caught it**, in rising order of cost:

* **Port a function's tail with its loop, or say at the loop that you did not.** The omission
  is invisible because `grain_labour_estimate` is a *complete-looking* function: it takes the
  right arguments, returns a sensible type, and reads as finished. A one-line
  `// NOT PORTED: the tail's four writes` at the return would have made the next reader's
  question *"why not?"*
* **Ask what *reads* the thing the comment says a pass is for.** The paragraph names the panel
  forecasts as the reason for the second round. Nothing in the workspace read a panel forecast,
  565: and nothing anywhere said so. That is `docs/decisions.md` C30 and the `farm_style` case from
  a third direction: **a producer that is protected and a consumer that is absent look
  identical from the producer's end** — and here even the *explanation* was written from the
  producer's end.
* **Count the pictures.** `docs/draws-map.md` had this counted as a missing draw call hours
  before the report arrived. That is the instrument, and it worked; but note what it counted —
  a *call site in the painter*, not a value in the simulation. It found the right defect for a
  reason one level away from the cause.

**And a second instance the same evening, with the same shape and a worse outcome.**
`docs/formats/maps-layers.md` §5.5 carried, as **[V]**: *"The third parameter is dead … all
sixteen call sites pass zero — including the two that forward a parameter (`FUN_00469D21`,
whose only callers are `Grain_SeasonTick` and `Herd_UpdateCrowding`, and both pass `'\0'`)."*
There are twenty-four call sites; `Herd_UpdateCrowding` passes zero and **`Grain_SeasonTick`
passes a computed variant**, which is the only thing in the picture that says how grown a
wheat field is. The renderer dropped the term *because the document said to*, and a player
said *"the wheat fields don't show the wheat growing."*

That one is already covered by *name the branch* — a true statement about one of two callers,
promoted to a statement about both —
together make the sharper point: **a document is an input to the code, not only a record of
it.** A wrong `[V]` does not fail to help; it actively produces the defect, and it does
so through a careful person who checked the reference. The correction log knows this about
itself (*"the correction log can be wrong, and it is believed harder than anything else"*);
the format documents have no such warning on them.

## The correction log can be wrong, and it is believed harder than anything else

The five failures below this heading are tools returning clean, plausible, wrong answers. This
one is different in kind and worse in consequence: **`docs/decisions.md` — the artefact this
project consults when documents disagree — produced a correction whose own central claim was
false, and used it to retract an entry that was right.**

What happened, in order:

1. A player reported a convenience of ours as a bug: *"if you click anywhere on grass it opens up
   the tax window too."* The instruction was to remove it.
2. Reading `Map_Click` (`0x0043CE1A`) turned up what looked like a free-standing final arm — *a
   click on a tile whose county is not the selected one, carrying no unit, selects that county and
   604: recentres* — and the conclusion drawn was that the fix asked for was too big: the original
   selects, it just does not open anything.
3. That reading was written up as C61, **C58 was edited in place to apologise for its claim that
   `Map_Click` has no county-selection arm**, `screens/map/mod.rs`'s module header was rewritten around
   it, and two tests were rewritten to assert it. All of it passed.
4. It is the **prologue of the industry branch**, guarded by tile flag `0x80` *and* by the county
   being the local player's. `Map_Click` has no such arm; its three writes to `g_selectedCounty`
are in the village, industry and merchant branches, as C58 said.

**Why this is worse than a wrong tool.** A tool's output is treated as a lead. A correction is
treated as settled — the whole point of the log is that it outranks prose written earlier, and
C58's flat statement is what stopped anyone re-reading `Map_Click` for weeks. A wrong
correction therefore propagates *further* than the mistake it replaced and is *harder* to
dislodge, because the next reader finds a numbered entry that says the question was already
asked and answered. It had already reached three documents and two tests before it was caught.

**What caught it was re-reading the decompilation before committing, and nothing else could
have.** Not the tests — they were rewritten to agree. Not the citation lockfile, the figures check
or the symbol check — all five checks were green, and none of them knows what a binary does. Not
review of the prose, which was internally consistent and cited four addresses. The only defence
that exists is opening the function again, at the end, and reading it against the claim rather
than for the claim.

So, as a working rule: **a correction that retracts an earlier correction re-reads the primary
source at the moment of writing, not at the moment of deciding.** The gap between deciding and
writing is where the draft's own confidence accumulates. And an absence is evidence only in
proportion to how hard it was looked for — three greps that found nothing became a claim, one grep
that found something overturned it, and a fourth reading overturned that. `docs/method.md` §4.

One consolation worth recording, because it is the reason to keep doing this: the corrected fix is
634: *smaller* than the fix that was asked for, and the fix that was asked for was smaller than the one
the draft proposed. Every time this project has read the binary, not reasoned from
an absence, the answer has been less work than the guess.

## C-numbers: the protocol assumes a serial writer, and we have eight

Six collisions now. The first four produced the protocol — *read the file, take the highest
641: number, add one* — and the fifth happened anyway. The sixth was today, three ways at once:
three branches all correctly read `C60` as the highest and all correctly chose `C61`. **Not one
agent did anything wrong.** They branched from the same head within an hour of each other, which
is what we asked them to do.

That is the diagnosis, and it rules out a whole class of answers. *"Check the highest number
first"* is already the protocol. *"Check again before committing"* loses to any branch that
commits after you check. No instruction fixes a race, because the thing being raced for is
allocated by reading a file that the other racer has not written yet.

### Why allocating at write does not work either

The obvious repair is a registry — a `NEXT` file, or reserved ranges handed out when an agent is
spawned. Both were considered and both are worse than they look.

- **A `NEXT` file** moves the race without removing it: two branches read `61`, both write `62`,
  and now the registry conflicts as well as the log.
- **Reserved ranges** — a block of five consecutive numbers to one agent, the next five to the
  next — do remove the race, because the coordinator allocates them serially at spawn time. They
  cost **gaps**. An agent given five numbers and using two leaves three permanently missing, and
  a numbered log with holes in it invites every future reader to ask what was deleted. The log's
  value is that it reads as a complete sequence.
- **One file per correction** — one markdown file per entry, named for its number and assembled
  by a script — makes the
  collision *mechanical*, not textual — two files claiming the same number is trivially detectable —
  but it does not prevent it, and it costs a build step on the project's most-read document.

### The proposal: assign at merge, not at write

**Move the numbering to the one moment when serialisation is already free.** There is exactly one
serial actor in this system and it is the merge: one integrator, one `main`, one commit at a
time. Everything upstream of that is concurrent by design.

So an agent never chooses a number. It writes a **placeholder**:

```markdown
**CNEW-hover — We reproduce artwork and skip behaviour.**
```

and cites it the same way in its own prose — `docs/decisions.md` CNEW-hover — for as long as the
branch lives. The suffix is the agent's own word, needs to be unique only within its branch, and
exists so that one branch can add several corrections.

At merge, `node tools/decisions/corrections.js --assign <TAG>` numbers **one** placeholder: it
takes the next free number in that placeholder's own series, rewrites **every occurrence across
the whole tree** — headings, prose, `symbols.json` comments, Rust doc comments, test names — and
relocks the citations. One command per tag, run in the order `--check` lists them. *(This
paragraph described a tag-less `--assign` that walked every heading at once for a long time
before any `--assign` existed; the integrator did the job with `git grep -l | perl -pi`. What
exists now is described under* The tool as it is *below.)*

692: **And the check that makes it structural, not a habit: `--check` fails if any
placeholder, of any family, survives anywhere on `main`.** That is what stops the placeholder
from being merged unassigned, and it is the whole enforcement. It goes red on ordinary work, in
CI, on the commit that would have introduced the problem — which is the property *"read the
highest number first"* never had.

698: What it costs: an agent's own branch reads `CNEW-hover` instead of `C61` while the work is in
flight. That is a branch, and it is correct that a number which has not been allocated does not
appear.

**It covers `docs/bugs.md`'s B-numbers in the same change, because they are the same race with
less protection.** Three branches took overlapping B-numbers tonight — two claimed `B64` and two
704: claimed `B65` —. `BNEW-<slug>` and the
same `--assign` pass.

**The detail that makes the case is that one collision merged silently.** Two branches both added
a `C63` heading, at different offsets in `decisions.md`, with unrelated text around them. Git
saw two additions in different places and took both — no conflict, no marker, nothing to resolve.
The duplicate was found by grepping for it afterwards, on a hunch. So the failure is not *"someone
forgot to check"*; it is **the tool that should have refused had no reason to**, and a convention
policed by human attention is exactly what fails that way. A placeholder cannot collide silently,
because there is nothing to collide.

What it does not fix, said plainly: **two agents writing corrections about the same thing.** That
is a content collision, not a numbering one, and no tool resolves it — it is the coordinator
knowing what is in flight. Today's three were three different subjects that happened to
want the same integer, which is the case this removes entirely.

### The tool as it is

`tools/decisions/corrections.js`, with `crates/l2-testkit/tests/corrections_tool/main.rs` building
small trees to break it. Built after a merge of six branches where the integrator closed three
gaps by judgement, each of which is now a rule:

**It knows six series, not one letter each.** A series is a *(log, letter)* pair:

| log | letter | what | entry forms |
|---|---|---|---|
| `docs/decisions.md` | `D` | decisions | `**D<n> — title**` |
| | `C` | corrections | `**C<n> — title**` |
| `docs/bugs.md` | `B` | §2, reproduced | `### B<n> — title` or `\| **B<n>** \|` (`~~` for retracted) |
| | `N` | §3, not reproduced | the same two |
| | `S` | §4, surprising and not a bug | the same two |
| | `D` | §5, dead code | the same two |

737: **The D-series exists twice and the two are unrelated**, so a `D` placeholder is numbered in the
log its entry is written in — by letter alone, the first dead-code placeholder would have become
decision D12. The tool once scanned for `C` and `B` placeholders only, and a `D` one reached a
merge with nothing reporting it. Every one of the six is duplicate-checked now, and **the first
run of that found two `B69`s on `main`** — a §2.5 table row and a §2.10 heading, merged fifty
minutes apart — which the quirks catalogue, holding one disposition for both, had passed. The
later arrival is B100 — B99 had been taken by the turn timer's row by the time this merged.

**A line that looks like an entry and is not in the log's form is an error naming the line**,
not a skip: a Markdown heading in `decisions.md`, a hyphen, en-dash or colon where the em-dash
belongs, a double-encoded em-dash, the wrong heading level, a cell closed tight. Two branches
wrote `## <placeholder>` headings the tool could not see, and C146's double-encoded em-dash made
its correction vanish the same way (C147). An invisible entry is worse than a missing one,
because the next free number is computed without it. Prose that opens with a bold id —
`**C12's failure mode…`, `**B4** the empire tax`, `| **D1 →** |` — stays quiet; the tests pin
those shapes too.

**`--check` reports placeholders last**, after every other rule has run, grouped by tag, with the
755: entry that defines each, the command that assigns it and the number it would take — and a letter
no log numbers, or a tag nothing defines, is reported as such. On a branch, which carries its own
placeholders by design, that means the placeholder list is the *only* failure when everything
else is right; on `main` it is fatal. `--relock` works with placeholders present.

760: **`--assign <TAG>`** refuses, changing nothing, when the tag is not in the tree, is not a
placeholder, is defined by no entry, or when **any** entry-shaped line in either log is
malformed — the refusal that stops it computing a number from a log it cannot fully read. It
also refuses when the tree already holds a dragged or left-behind citation, because it relocks
and a relock is how a drag gets accepted. Otherwise it **reads and writes bytes**: a tag is
ASCII, so it is replaced inside the file's bytes without decoding the rest, and a BOM, a CRLF or
a byte that is not UTF-8 goes back out as it came in. It checks that the byte replacement found
exactly what the scan found before writing anything, and it will not take `<tag>` out of the
middle of `<tag>-longer`.

    node tools/decisions/corrections.js --check              # lists each placeholder and its command
    node tools/decisions/corrections.js --assign CNEW-hover  # the next free C-number, tree-wide, relocked

773: **The quirks catalogue sees placeholder rows.** `quirks_catalogue.rs` accepted digits only, so a
branch's new `docs/bugs.md` §2 row was invisible to it: the branch was green, and two tests went
red at merge once the row had a number. A placeholder is an entry now, so the branch goes red
until `DISPOSITIONS` carries the placeholder, and `--assign` renames that line in the same pass
as the document.

### Why this is written here

Two renumbers were done by hand today. Both were clean because no file cited both colliding
entries — the citation lockfile caught one drag on `symbols.md:307` and confirmed the rest — but
that was luck about which documents the branches touched, not a property of the method. The next
784: collision will be between two branches that both cite the same file, and the hand method's
failure mode there is a citation quietly renumbered to point at the wrong correction: a wrong
pointer into the log the project trusts most, which is the failure mode recorded above under
*The correction log can be wrong*.

## A check that passes for an accidental reason looks exactly like one that passes

791: Three of these in two days, and the third was found while writing this section,
best evidence that it is a class, not a run of bad luck.

> **A check that passes for an accidental reason is indistinguishable from one that passes for
> the right reason, until the accident stops holding.**

It is the sibling of the near-miss rule above. That one is about a check firing outside its
remit; this one is about a check *not* firing, correctly, for a reason nobody chose.

**One.** `JSON.parse` threw on a hand-merged `symbols.json` and that throw is the only reason
the nine misaligned hunks were ever looked at. The check was for syntax. What it caught was a
semantic disaster, and only because the misalignment happened to produce unbalanced braces.

**Two.** `docs/arms.json`'s marker rule was `status == "reproduced"` — and it was correct, and
it was correct *by accident*. Every invention on file happens to have been **removed**, so no
invention needed a marker, so filtering on `reproduced` alone lost nothing. An invention we
chose to **keep** — which the file explicitly supports, with a `removed` field to say so —
would have needed a marker and slipped straight through. The check was right about today's data
for a reason unrelated to what it was written to guarantee.

**Three, found while writing this**, and it is the sharpest of the set, so it has the section
of its own above: the build-stamp test that counted non-background pixels, passed, and passed
just as well with the line that draws the stamp deleted. See *Ablate the line, or you have not
tested anything*.

### The defence

**State the predicate the check *means*, then implement that** —
something that happens to agree with it on the data in front of you.

* The marker rule now reads `Record::in_the_tree()`, and that method is the sentence *"is
  there code to mark?"* written out: reproduced, dead-reproduced, or an invention not yet
  removed. `status == "reproduced"` was a proxy that agreed with it on today's rows.
* The stamp test now draws the page, copies it, draws the stamp **again** on the copy, and
  825: requires the two to be identical. Text is an opaque blit, so a second draw over itself
  changes nothing — but only if it was there the first time. That is the claim exactly, with
  no threshold and no knowledge of what else is on the page. Deleting the draw call moves
  1,234 pixels.

830: And the operational half, which is cheap and caught the third instance:

**Ablate the thing the test is about, and watch it go red.** Not the code near it — the exact
line the assertion claims to be about. A test written against a passing tree has never been
observed failing, and until it has, "it passes" is a statement about the tree and not about the
test. Two of the three above were written by someone careful and neither would have survived
thirty seconds of this.

## A file that looks like data is usually a claim, and claims cannot be merged positionally

Six branches landed in one evening. Nine merge defects came with them and **all nine are the same
mistake**: a file was treated as a list of lines or a list of entries, when what it held
was an assertion about the current state of the project. Every one of them was made by choosing
the resolution a careful person would choose.

This is written once, with all nine in front of it, because it will be harder to assemble later
and because the individual anecdotes each look like carelessness. They are not. The pattern is
847: that **positional merging is correct for data and wrong for claims, and the two are
indistinguishable by looking at the file.**

### The near-miss, which is the centrepiece

`docs/symbols.json` came up with nine conflict hunks. Every one of them was a lie: **the `ours`
body of each hunk sat under the *wrong entry's* `name`.** Git had aligned two arrays that were
ordered differently, so the diff paired unrelated records —

```
      "name": "Move_BuildCostMap",
<<<<<<< ours
      "comment": "Places the weapons industry site (record 2) on the county tile whose flags…"
=======
      "comment": "Rebuilds the 64x64 i16 campaign movement cost map g_moveCost from the map…"
>>>>>>> theirs
```

— `County_PlaceBlacksmith`'s comment, offered as a candidate body for `Move_BuildCostMap`.
`County_Reset`'s, offered for `Path_SearchSiege`. Nine of them.

Resolving those textually — taking a side, taking the union, taking the newer — would have
produced **a symbol database that parses, reads plausibly, and lies about what functions do**, in
the file this project consults to decide what the binary is. It is the stolen doc comment from
`FIELD_TRAMPLE_OFFENCE`, at scale, in the worst possible place.

The correct resolution was not textual at all. `symbols.json` is a database keyed by address, so
it was merged **by address**, three-way against the merge base: base 757 entries, ours 806,
theirs 763, merged 812 — and **no address had been changed on both sides**.
conflict. There were only two arrays in different orders.

### What saved it was `JSON.parse` failing, and that is a near-miss report

880: The union produced invalid JSON, the parse threw, and the throw is the only reason any of this
was looked at. **A syntactic check caught a semantic disaster by luck.**

That is worth its own rule, because the temptation is to file it as the check working:

> **When a check catches something outside what it was built to catch, treat it as a near-miss
> report, not a success.** The next instance will differ by whatever made this one syntactically
> invalid, and nothing will fire.

Had the two orderings happened to produce valid JSON — one entry moved, not several, or a
conflict that closed its braces evenly — the file would have been committed, `symbols_md.js` would
891: have regenerated `symbols.md` from it without complaint, both would have been green, and the
project would have carried a wrong comment on a right address until somebody read that function
again, so `symbols.json` and
`hypotheses.json` now merge by key through a driver, not by anyone remembering to.

### The nine, and what caught each

| # | what was merged positionally | what it asserted | what caught it |
|---|---|---|---|
| 1 | `--theirs` on `shells.rs` | *these screens are not built yet* — so an older copy **un-builds** them | a test: *"CASTLE is both a shell and a screen"* |
| 2 | union of two `symbols.json` runs | *this hypothesis was promoted* — a union turns a move back into a copy | `symbols_md.js`, four times, one at a time |
| 3 | a test naming shell `0x1B` | *`0x1B` is still unbuilt* — it broke **on success** | itself, correctly |
| 4 | union across `GATED_TOTAL` | a scalar, not a list | the compiler: two `const`s of one name |
| 5 | two `C63` headings | *this correction is C63* | **nothing.** A grep on a hunch |
| 6 | `FIELD_TRAMPLE_OFFENCE`'s doc | *this prose describes this constant* | **nothing.** Reading it |
| 7 | two `quirks:` initialisers | a field, not a line | the compiler |
| 8 | nine `symbols.json` hunks | *this comment describes this address* | `JSON.parse`, by luck |
| 9 | B-numbers in `docs/bugs.md` | *this number identifies this defect* | the quirks catalogue |

Three of the nine had **no defence at all** and were found by a person looking. Two were caught
911: by the compiler,
The rest were caught by checks built for other purposes.

### The B-numbers, and an instruction that read as complete

`docs/bugs.md`'s numbering was **already broken before this evening**: `B66` named both §2.3's
`Diplo_ActionAllowed` row and §2.6a's sound-flag entry, and nobody had noticed. A branch then
added a third `B66`, and another added six defects as **table rows** numbered `B62`…`B67` that
collided with six existing `###` headings.

That last one was missed at its merge because the check run was *"grep the `###` headings"* — and
the collision was in table rows. The instruction that prompted it was *"note the B-number range
against what the other branches took"*, which reads as complete while naming no artefact.
**An instruction to check something is only as good as its specificity**; "check the B-numbers"
and "check every line matching `^### B` and every line matching `^| \*\*B`" are different
instructions, and only one of them can be followed wrongly.

Quote this beside `C61`'s **six** claimants when arguing for assigning numbers at merge. The
C-space collided six times in one day and was noticed every time; the B-space had been quietly
wrong for weeks and was noticed only when a check finally read the file **as data**
as prose.

### What to do instead

1. **Merge keyed files by key.** `symbols.json` and `hypotheses.json` have a merge driver.
   Anything with a stable id and an unstable order belongs there too.
2. **Ask what the file asserts before choosing a side.** If the answer is *"what is true right
   now"* — what is unbuilt, what was promoted, which number means which defect — then neither
   939: side is safe and the resolution has to be derived, not picked.
3. **A union is safe for a list and unsafe for anything else.** Twice tonight a union was applied
   across a scalar, in files whose surrounding lines were a list.
4. **Compile, then test, then read.** The compiler caught two of these, tests caught two, and
   three had no automated defence whatever. Budget the reading.

### The worked example: a counting file that would have been silently deduplicated

947: Everything above is abstract until it costs something. This is the one that nearly did, and it
is three separate failures stacked, each of which would have been enough on its own.

**One: the file was not registered.** `docs/arms.json` — the file whose entire purpose is
counting, with three agents writing it concurrently — was **not in `.gitattributes` at all**. The
merge driver had been built, tested, falsified and wired up the day before, specifically for
953: files of that shape, and the newest and most exposed file of that shape was outside it. The gap
was not in the mechanism. It was in the **registration**, which is the half nobody checks,
because a mechanism that works is satisfying to verify and a list of files it applies to is not.

**Two: the key was wrong, and wrong in the direction that deletes.** `KEY_FIELDS` preferred
`addr`. `arms.json` carries both `id` and `addr`, and it has **42 records across 23 addresses**,
because one function can hold several input arms — `Screen_FrameInput` alone holds six. Keyed on
`addr`, the driver would have kept one record per address and **discarded 19 in silence.**

Sit with what that output would have looked like: a smaller file, internally consistent, every
remaining record correct, valid JSON, passing every check that existed. **A deduplicating merge
is the worst possible failure for a counting file**, because the thing it destroys is the count,
and a count has no local evidence of being wrong. The 1:1 percentage would have been
computed from a smaller denominator, and nobody would have had any reason to look.

A keyed merge assumes its key is unique.
Nothing anywhere asserted that. `merge-json.js --check` now refuses any array whose chosen key is
not unique — which closes it for every file at once, including the ones nobody has written yet,
and turns a silent deletion into a loud refusal that names the collisions.

Reverting the key order now reports *"arms has 15 duplicate keys — merging it would DELETE the
duplicates, one per collision"*. Before, it reported nothing and returned a smaller file.

### Two artefacts that must agree, and when that pattern lies

Most of this document recommends duplication: a number in a document against a number derived
from the tree, a citation against the heading it names, a marker in the code against a record in
a file. It is the pattern that has caught nearly everything.

It has a failure mode:

> **Two artefacts that must agree is the pattern that catches things. Two artefacts that must
> agree *and are maintained by the same person, at the same time, for the same reason* is the
> pattern that lies.**

Both copies get updated together, by someone holding one intention, and they agree because they
were written to agree. The check passes
firmly and means nothing.

So the test for the uniqueness rule **shells out to `merge-json.js --check`**
reimplementing `KEY_FIELDS` in Rust. A Rust copy of the key rule would be a second list, edited
by whoever edits the first, in the same session, for the same reason — which is the
failure this whole area is about, reproduced inside its own remedy. The driver's logic is what a
merge will use, so the driver's logic is what has to be asked.

The discipline: before duplicating, ask **who maintains each copy and when.** If the answer is
"the same person, in the same commit", you have not built a check — you have built two places to
make the same mistake.

## Prefer a shape that cannot be wrong to a check that notices when it is

Almost everything in this file is the same remedy: **two artefacts, maintained by different
work, that must agree.** `symbols_md.js`, `figures.js`, the citation lockfile, the test census,
the encode/decode scanner. It is a good pattern and it has caught real things.

It is the second-best pattern.
leaving it implied.

The best one is a **construction in which the failure cannot be expressed**. The case that
produced this heading: `l2-scenario` builds a `County` from a `.sav` by assignment onto a
1013: default — `c.farm_style = s.farm_style;`, forty-odd lines of them — so a field nobody remembered
is left at zero and nothing anywhere says so. That is how `County::farm_style` (C62) and
`Unit::mission` both survived. A **struct literal with no `..`** makes every one of those
omissions a compile error: rustc refuses to build until every field is named, permanently, with
nothing to maintain and no scanner that can be fooled by a name.

Compare the two honestly. The source-text check (`crates/l2-testkit/tests/encoding/main.rs`) reads
text, resolves names, and got two resolutions wrong on its first run. The struct literal cannot
be got wrong,. **Where that option exists we
1022: should take it**, and the check should be reserved for the boundaries where it does not —
is most of them, because the compiler cannot read `L2.eng` or a decompilation.

The general form: **ask whether the mistake can be made unrepresentable before asking what would
notice it.** A check is what you build when the answer is no.

And a corollary about naming, learned from `docs/arms.json`'s `dead-reproduced`: when a value
names something that should not happen, make it **awkward and assert it stays empty**. A
category that quietly acquires members is how a finding becomes a bucket — the first entry is a
discovery, the tenth is a fact of life nobody reads any more. The status exists so the case
cannot hide inside a neighbouring one; the assertion exists so that using it costs a decision.

1034: ### The producer is protected and the consumer is not, and everyone reviews from the producer's end

The struct-literal fix above was aimed at an importer that had lost a field: `County::farm_style`
was read by the rules and written by nothing on the import path for months. The obvious repair is
1038: to make the importer build its `County` with an exhaustive literal, so a new field stops the
build.

**Ablating it said something better than that.** Adding a field to `CountyState` now produces
**three** compile errors, and two of them were already there: *both* constructors — the map
1043: reader and the save reader — had used exhaustive struct literals all along. The write side had
been protected from the beginning. The entire hole was on the **read** side, and that is exactly
where `farm_style` fell through: something wrote it faithfully and nothing ever picked it up.

> **A producer that must fill every field, and a consumer that may ignore any, is a shape that
> looks completely safe from the producer's end.**

1050: And the producer's end is where everyone stands. A reviewer opening an importer reads the
constructor, sees every field named, and concludes the data is carried — when what they have
verified is that the data was *assembled*. Rust makes this asymmetry easy to fall into, because
a struct literal is checked for completeness and a field access never is; the same asymmetry
exists in every language with named construction and free access.

The remedy is to **destructure the source with no `..`** at the point of consumption, which turns
"did anyone read this?" into a compile error. `crates/l2-scenario/src/mod.rs` does it for
`CountyState` and `RealmState`.

### Choose the smaller list, because noise is where an omission hides

The same fix had a second decision in it, and it is the one that generalises further.

`County` has **101** fields; `CountyState` has **46**. Destructuring the destination would have
been an exhaustive literal over 101 names, of which some seventy are derived, computed later, or
absent from the save — seventy lines of `x: 0` for a reader to scan past. Destructuring
the source is 46 names, **every one of which is something the file stored**, so the
question *"is this carried?"* is meaningful on every line.

> **Noise is where an omission hides. A check over a smaller list of things that all matter is
> stronger than a check over a larger list that is mostly filler** — even though the larger one
> covers more.

This is the same instinct as making `dead-reproduced` awkward and asserting it stays empty, which
is why the two sit together: both are about refusing to let a check accumulate members it does
not mean. A list that acquires filler stops being read, and a list that stops being read is a
list that no longer checks anything — it just fails to compile in the right places, which is not
the same thing as being understood.

The practical test, before adding to any exhaustive list: **would a reader scanning this line
have a real question to answer?** If most lines have no question, the list is measuring the wrong
set.

## A correct experiment can produce a wrong inference, and it has four times now

Every other entry under this heading is a tool returning **wrong output**. These two returned
**right output** and were read to mean something it could not mean, which is a different failure
and needs its own line, because the defence is different: checking the output harder would not
have helped either time.

1091: **The first was the county-selection arm.** `Map_Click` was read, an arm was found, and the
conclusion drawn was that C58 had been wrong to deny it. Every step of the reading was accurate
1093: except the one that mattered: what was found was the *prologue of the industry branch*, and the
question being answered — *"is there a free-standing selection arm?"* —
the text. `docs/decisions.md` C61.

**The second was worse, because it was an experiment designed on purpose.** The question was
whether the lockstep digest covers a given field. The experiment was to remove the field from the
1099: encoder and count the tests that went red. Both of us — the agent running it and the coordinator
who asked for it — read the red tests as an answer. **They could not be one.**
`Canonical::hash_of` *is* `value.encode(&mut c)`, so the digest is a projection *through* the
encoder: a field absent from the encoder is absent from the digest on every peer identically, and
removing it makes round-trip assertions fail. The experiment measured whether the
round trip works. It was structurally incapable of measuring the thing it was run to measure, and
it returned a clean number either way.

The sharp illustration, which is the thing to quote:
`ten_seasons_from_a_reloaded_game_are_the_same_ten` **passes with the field dropped.** It saves a
played game, reloads it, and compares the digest after each of ten further seasons. The reloaded
kingdom *is* a different world — a mine with no ore behaves differently from one with
ore — and ten seasons of that divergence never moved the number.

1113: **And a third instance, from the same hour and the same person, so this is a pattern
and not two accidents.** The instruction that followed was to prove the new encode/decode check
by making it fail on `Unit::mission`. It was wrong twice over: that field does not exist on
`main` at all — it arrives with an unmerged branch — and *neither* `mission` nor `farm_style` is
an encoder omission, so no encoder check could ever have caught either. Both were dropped by the
**importer**, building a struct from a `.sav`, on a path that never touches `encode`. The proof
target was neither available nor in scope.

None of these was carelessness. All of them were **confident reasoning about which check covers
what, done without checking** — which is the same act the whole project exists to avoid when the
subject is `Lords2.exe`, applied to our own tools instead, where it feels like knowledge rather
than inference because we wrote them.

1126: **A fourth, and the first that travelled in a BRIEF, not in a tool's output.** An agent
reported that retreat and autocalc discard casualties. True — of the solo retreat arm. It was
repeated as *"every casualty we have fought so far is unkilled"*, which is a statement about the
seam, and briefed onward in that form. Measured: the forty-turn run kills 1,951 men and always
did, `Battle_WriteBackCasualties` has five call sites, we implement it, and ablating ours turns
three tests red. **A true statement about one branch, promoted to a statement about the**
1132: **subsystem** — and the promotion happened in prose, between people, where none of the tool
checks in this file can reach. `docs/decisions.md` C71.

The defence is the same one and it is cheap: **name the branch.** "Retreat, in single player,
discards casualties" is the same finding and cannot be promoted by accident, because the scope
is in the sentence.

**What generalises.** Before running an ablation, say which artefact you expect to fail and *why
it is downstream of the thing you are testing*. If the answer is "the digest", check whether the
digest is built from the thing being ablated. And read *which* tests went red, not how many — the
1142: count was wrong too (two, not eight), and the count being wrong was the less important error.
`docs/decisions.md` C65.

## The sixth wrong answer was the first flattering one

A new tool's first run reported **twenty-four findings**. Twelve were fields of
`l2_formats::save::Unit` — the raw `.sav` record — reported missing from a codec belonging to
`l2_kingdom::unit::Unit`, a different type that happens to share a short name. Six more came from
the same collision on `Order`, resolving an `l2-net` test fixture to `l2_kingdom::trade::Order`.
The scanner had resolved a name by its last path segment across the whole workspace.

**Twenty-four findings look exactly like a productive new tool**, and that is the entire lesson.
The five failures above this heading all *understated* or *misdirected* — a symbol silently
1155: dropped, a count that read as success, a rename that quietly unhooked a table — and the defence
against those is suspicion, which people can be asked for. This one **overstated**, and it
overstated in the direction nobody is inclined to doubt: it told us we had a lot of bugs.

1159: It is the exact twin of the `ApplySymbols` tally that reported two failures for one, and the pair
makes the point better than either alone. *A tool that overstates is one people learn to
discount*, and a tool discounted is a tool switched off. Both directions cost the same thing in
the end; only the flattering one buys a few days of feeling productive first.

1164: It was caught by **reading the failures instead of counting them** — opening the first row and
asking which `Unit` it meant. That is the only defence that has ever worked, and it is expensive
enough that it has to be spent deliberately: read the first three findings of every new tool's
first run, in full, before believing the total.

The fix was to require **same-crate resolution**, and to make *unverifiable* a distinct verdict
from *absent*: a type whose codec exists but whose struct will not resolve in its own crate is
now reported as a type the check makes no claim about, listed by name in `UNVERIFIABLE` and
asserted so the list cannot grow quietly. Four types are on it, each read and classified — a
tuple struct with no named fields and three enums. **A growing unverifiable list is itself a
signal**, and it is a signal only if somebody has to look at it.

## A tool that degrades silently is worse the more people use it

`tools/oracle/decompile-all.ps1` **exits non-zero** when `ApplySymbols` or
`ApplyRecords` drops an entry, and `tools/symbols/symbols_md.js --check` refuses four
shapes of `signature` field that cause it. Both exist because of the same two incidents,
two days apart:

    void __cdecl Setup_SetOption(int, int)   a calling convention, which Ghidra's
                                             C parser rejects outright
    int g_goodsStall[14][5]                  a data table filed under "functions"

1187: **Each was dropped on every rebuild, and the pipeline reported success both times.** The
name never reached the corpus; `ApplySymbols` printed one line among a hundred; an
integrator happened to read it.

The general shape is the one this file keeps returning to: **the dangerous tool failure is
not the one that errors, it is the one that returns a clean, plausible, wrong answer.**
Five of the six tool defects logged in `docs/decisions.md` are of that kind — `litNum`
reading `''` as the letter b, `anchor.js` inventing six coherent screen ids, a rename
unhooking `ENG_CALLS`, and these two. Every one produced output that looked right. Every
one was caught by something *external* contradicting it, never by the tool noticing itself.

So when you write or change a tool here, ask what it does when it half-works, and make that
case loud. And do not hesitate to add a check to a tool other people depend on: **a tool
that degrades silently is worse the more people use it**, so shared use is the argument for
the check and not against it.

## Rule 5 needs a check, and here is the argument for which one

`CLAUDE.md` rule 5 — *if we implement a feature, find its equivalent in the binary's functions* —
arrived because a player noticed that the march preview appears on **hover** in the original and
only after the click in ours, and then, a minute later, that an army cannot be deselected. Both
are arms of `Screen_FrameInput`'s screen-`0x10` ladder. Neither had been looked for.
`docs/decisions.md` C61 has the whole of it, including the count: **80 of 185 input arms
reproduced, 43%.**

**A sixth rule in a list has the enforcement of the ones that failed.** The numbering protocol was
written after four collisions and did not prevent the fifth. `git add -A` is blocked by a hook
because guidance was not enough. Five of six tool failures on this project returned clean,
plausible, wrong answers, and *every one* was caught by something external contradicting it, never
by the tool noticing itself. So the question is not how to phrase rule 5. It is what goes red.

1218: ### What was proposed, and the two halves of it that do not survive

The proposal was: a per-screen inventory of the original's input arms, and a census that goes red
when a screen graduates out of the shell table without one.

**The inventory is right.** `Screen_FrameInput`'s `0x02` arm running six sidebar guards was found
by enumerating, not by reacting to a report, and enumerating three screen groups is what
produced the 43% at all. Nothing else this project has tried finds an arm nobody asked about.

**The trigger is wrong, for a measurable reason.** "Red when a screen graduates" fires once per
screen, at graduation. Every miss found today is on a screen that graduated weeks ago; the check
1229: would have caught **none of them**. Worse, it can only ever go red once — after the file exists it
is green for ever, however wrong its contents become. It checks that a document was created, which
is the same class of guarantee as checking that a rule was written down.

**And one premise behind it is false, which is worth stating because it was cheap to test.** The
proposal leans on *"every screen module already opens with the painter's address; that convention
exists and is followed."* Measured across `crates/l2-game/src/screens/`: **10 of 16 modules cite at
least one address in their module docs, 6 cite none** — and `map.rs`, which had a 74-line header
and is the screen both of today's misses live on, was one of the six. The convention is not
followed; it is *mostly* followed, which is exactly how an unchecked convention decays. (It now
cites eleven, in a table of its arms.)

Also worth saying plainly: **a check that a feature cites *an* address is theatre.** An address in
a comment proves a lookup happened, not that a function was read. C61's own first draft cites four
addresses and is wrong about all of them.

### What is proposed instead

The three checks that have worked here — `symbols_md.js`, `figures.js`, `corrections.js`
— all have the same shape, and it is not "a file must exist". It is **two artefacts maintained by
different work, that must agree.** A number in a document versus a number derived from the tree. A
citation versus the heading it names. That shape goes red on ordinary work, in both directions,
which is the whole point.

**The schema is being authored by the battlefield agent, not here.** It has ~50 records
blocked waiting on one and is the file's largest single contributor, which makes it the right
author; this section is the requirement, not the design. Three constraints are non-negotiable
and they are the reason the file exists at all: set equality in both directions between
`// arm: 0x…` markers and `reproduced` records; an **invention** must be representable and
countable, not absent; and a **dead** arm must be distinguishable from a missing
one. Amend that schema once if it is wrong, and say why — two schemas would be worse than
either.

The shape asked for, so the requirement is concrete —

```json
{ "screen": "0x10", "addr": "0x004A8E0B", "gesture": "hover",
  "what": "the route under the cursor, recomputed when the hovered tile changes",
  "status": "reproduced", "by": "MapScreen::update_hover_path" }
```

`status` is `reproduced`, `absent`, `ours` or `n-a`, and `n-a` must name the unbuilt subsystem.
Then four checks, in rising order of cost and value:

1. **Every `addr` resolves in `docs/symbols.json` or `docs/hypotheses.json`.** Cheap, and it is the
   difference between citing an address and citing a *function somebody named*. An arm cannot be
   invented from a plausible-looking constant.
2. **Every `by` names a symbol that exists in the crate.** Cheap, and it catches the failure this
   project has already had once: a rename silently unhooking `ENG_CALLS`. An arm that stops being
   reproduced because its function was renamed or deleted goes red the same day.
3. **The two lists must agree, both ways.** Each handled gesture in a screen module carries a
   1280: one-line marker — `// arm: 0x004A8E0B` or `// arm: ours` — and the check is *set equality*
   1281: between the markers in the code and the `reproduced`/`ours` records in the file. This is the
   half that fires on ordinary work: a new handler with no marker fails, a marker with no record
   fails, a record claiming an arm nobody built fails, and a deleted handler fails. It is also the
   half that makes an invention **countable**, which is what a 1:1 goal needs — `ours`
   arms are a number that should go down.
4. **A per-screen percentage, printed by the census.** Not a gate: a number in `status.html` and
   the README, maintained by the script, that cannot drift because nobody types it. `figures.js`
   1288: already does exactly this for other counts, and the precedent there is *"a number that cannot
   drift beats a number that is checked"*.

The falsification condition, since a check that cannot go red is a rule: **check 3 fails if I add a
`RightClick` handler to `divide.rs` without an entry, and fails if I delete
1293: `MapScreen::update_hover_path` without one.** Both are one-line experiments and both should be run
before this is believed.

### What it costs, honestly

Enumerating three screen groups took three agents about half an hour of wall time each and
produced ~185 rows, most of which are verdicts, not research. Marking up the existing
handlers is the larger job — perhaps a day — and it is unavoidable, because check 3 is worthless
until the markers exist and check 3 is the only one that fires on ordinary work.

**So do not start with all of it.** The three groups already enumerated are `map.rs`,
`village.rs`/`job.rs`, and `county.rs`/`divide.rs`/`shells.rs`, and their tables are in C61 and in
the three agent reports. Land those, run the two falsification experiments, and let the census go
red for a screen that has arms in the file and no markers in the code. The battlefield's 49 arms
are the argument for doing this *before* that screen is built, not after — it is the only
screen group where the inventory would be written ahead of the code, which is the cheap direction.

### Counting arms cannot tell you whether a screen is reachable

**The audit's own method has a hole, and it was found the same way everything else was — by a
second enumeration from a different direction disagreeing with the first.**

The counts above come from reading each screen's input handlers. That is the right way to count
arms and it is the only way to count them. It cannot tell you whether anyone can ever get to the
screen. An exhaustive scan of every `mov byte ptr [g_screenId], imm8` in the binary can: 212
sites covering `0x00`…`0x45`, and **`0x28` is not among them**, nor is it assigned by any
decompiled function. It has a live-looking `Screen_FrameInput` arm and a live-looking
`Screen_Draw` arm, and both are dead code. So the battlefield is **three** screens — `0x29`
field, `0x2A` drag, `0x2B` outcome — and every arm audited on `0x28` counts toward a
denominator it should not be in.

1324: So the totals in C61 are provisional until the corrected denominator lands, and the corrected
one goes into `tools/figures/figures.js`, not into prose: it is about to be the headline
of `docs/plan.md` revision 5 and quoted widely, and 34 figures have already gone stale in four
documents by being typed. **A number that cannot drift beats a number that is checked.**

The general rule for the arms file: **every screen carries a reachability record beside its arm
records** — the addresses that write its id — and a screen with no writer is `dead`, which is
why `dead` has to be a distinguishable status, not an absence. Two enumerations from
different directions are what has caught things all evening; one enumeration is a claim.

### One entry here

Everything else under this heading is something going wrong, which makes the file read as a
catalogue of carelessness. `docs/bugs.md` B65 is the good case and belongs beside them.

Two of the village's eight animation counters are stepped every frame and read by **nothing in
the entire binary**. One of them has 21 states and `villani1.pl8` happens to have 21 frames.
That is exactly the shape of a finding: a number that matches, a plausible story available for
free, and a decompiled function that would have looked like evidence for it. The agent recorded
1343: the coincidence and **declined to build the story on it**, marking the counters dead and the
match unexplained.

1346: That is rule 4 working — *a plausible story assembled from decompiler output is not a finding* —
temptation wins.

### The rest of the pattern, recorded

Two other things fell out of the enumeration and belong here, not in a correction:

- **Right-click is the gesture we systematically miss**, because "right click exits" was learned
  early and generalised. The original uses that button for four different verbs — exit, cancel a
  drag, clear a selection, re-centre a panel — and 11 of the 22 right-button arms found are either
  missing or wrong. One is *wrong*, not absent: a right-click while carrying peasants leaves
  the village
- **Two documented claims were falsified by the enumeration**, both of the "verified" tier:
  `Village_DoubleClick` is *not* the only reader of `g_mouseLeftDoubleClick` (`FUN_0043BF07` and
  `Hotspot_Test` read it too, and `crates/l2-game/src/input.rs` repeats the claim), and screen
  `0x12`'s arm has no right-button test at all, so `battle.rs`'s right-click-to-Decline is ours.
  Neither was found by looking for errors; both fell out of reading a screen exhaustively. That is
  the argument for enumeration in one sentence.

## Every instrument we have measures inside a boundary somebody drew, and none can see the boundary

This is the most important entry in this file and it was the last to be written, because it is
only visible once you have several instruments and notice they share a blind spot.

Three statements, from three unrelated parts of the project:

* **The arms denominator is a *place*.** `docs/arms.json` reports a percentage of the input arms
  we reproduce, and that percentage is over *the functions somebody chose to mark up*. It was
  1374: labelled as coverage of the original's input handling. The measurement was correct and the
  label overreached — the honest form names the place: *"of screens 0 and 0x10, we answer 20 of
  25 gestures."*
* **A completeness check over a struct is only as complete as the struct.** The exhaustive
  `let RealmState { … } = r;` destructure cannot forget a field. It said nothing about `pairs`,
  because `l2_formats::save::Realm` had no `pairs` field to forget — the hole was outside the
  thing being checked exhaustively.
* `symbols_md.js` proves every symbol in the
  registry appears in the document. It has no opinion about whether the comment beside it is
  true, which is how `Wall_Collapse` carried *"surface 5 — rampart"* for weeks after the project
  had established that 5 is the bailey.
* `corrections.js` checked that
  every `C`-number in the tree names an entry that exists. A `C61 -> C63` renumber left two files
  1387: still saying `C61`, which by then was a different correction entirely, and the tool reported
  *"all citations resolve"* throughout — because they did. The lockfile now remembers **which
  heading** each citation was pointing at, so the entry moving under a citation is caught as well
  as the citation moving under an entry (rules 3 and 5). The pair is worth studying: the same
  renumber can drag one citation and strand another, and until both fields existed only half of
  it was visible.

1394: The shape is one shape. **Every check we have is exhaustive over a set, and the interesting
failures are outside the set.** A green check therefore licenses a statement of the form *"nothing
inside this boundary is wrong"* — and it is read, always, as *"nothing is wrong."*

, because an instrument that could see
its own boundary would need a larger boundary. What there can be is a **habit of naming the
boundary in the same sentence as the number**, so that the overreach has to be written down
is the reason that row is worth more than the figures above it: it says which instrument covers
it *and what that instrument still cannot see*.

Two consequences worth stating separately.

**A quantifier beats a list.** *"Every overlay we can build"* stays true when someone adds an
overlay; *"the seven overlays"* silently stops covering the eighth. Where a check must enumerate,
enumerate by construction, not by hand.

**And counting arms cannot tell you whether a screen is reachable.** An arm audit over a screen
nobody can open scores 100%. The boundary there is the word *screen*.

## A document that promises "until X" keeps promising it long after X

`crates/l2-view/src/text.rs` carried a header saying the interface draws its own letters *"until
The font had been decoded.
screen written after that point read the header, believed it, and reached for the 5×7 debug font
1418: — and the draw-call audit later found **six modules drawing nothing at all through the game's own
fonts or artwork**: castle, diplomacy, siege, job, menu, menubar. `menu.rs` drew the game's own
title as "LORDS OF THE REALM II" where `L2.eng` 11/0 says *"Lords of the Realm 2"* — a string
`tests/shell.rs` had asserted for weeks, on the first screen anybody sees. A player reported it
as *"the title screen is illegible."*

`docs/audit.md` F21 had **already flagged the stale bullet that comment cited.** The flag was
correct, it was filed, and it changed nothing, because flagging a document does not edit it.

1427: This is the fourth stale-document mechanism catalogued here, and the only one with a **tense** in
it. The others go stale when the world changes around a statement of fact; this one is a promise
about the future, and it expires *at a moment nobody is watching for* — the moment the promise is
kept. Nothing about the sentence changes.

> **A comment that defers work to a caller must name the caller. A comment that defers work to a
> future must name the condition that ends it — and something has to check that condition.**

Pair those two. They are the same defect: a sentence handing responsibility somewhere it cannot
be collected from. The practical rule is that *"until"*, *"for now"*, *"temporarily"* and
*"pending"* are all load-bearing words, and a header that uses one should either carry the check
that retires it or carry the issue number that will.

## A contradiction between two documents is invisible until someone needs both

`docs/hypotheses.json` had `0x0055307C` recorded as `g_rampartCellsBreached` — *"how many
rampart cells this siege has knocked through"* — with an honest caveat saying the mechanism was
1444: read and the purpose was a guess. That is right. `crates/l2-sim` called the same four bytes
`moat_flag`. That is wrong, and it is what the code ran on.

Both files were maintained. Both were checked. Neither was checked **against the other**, and
nothing could have brought them together: the hypothesis register is keyed by **address**, the
struct field by **name and offset**, and no instrument in the tree relates a field to the global
it mirrors. The project held a good reading and a bad reading of the same address for weeks.

What surfaced it was `symbols_md.js` refusing a promotion — *promotion is a move, not a copy* — a
rule written for bookkeeping reasons.
part:

> **Two artefacts that must agree is the pattern that catches things. Two artefacts that
> overlap catch nothing, and will disagree indefinitely.**

If two files can describe the same object, make one of them derive from the other, or make a
check that joins them on whatever key they share. Absent that, assume they already disagree,
and no way to get any.

## Merging is real work, and treating it as plumbing loses things silently

Three losses, all mine, all at merge, none caught by a test:

* A `--theirs` resolution **un-graduated castle building**, and another dropped `menubar`,
  `belongs_to_the_right_column` and `Transition::Reveal`. Tests caught the first. Nothing caught
  the second until it was looked for.
* A naive union **glued one function's doc comment onto a different constant** —
  `FIELD_TRAMPLE_OFFENCE`'s prose ended up over `CASTLE_TERRAIN`. Nothing caught that at all. It
  compiles, it reads fluently, and it is a lie in the one place this project treats as
  authoritative.
* A merge **lost an input arm** — the campaign minimap dropped out of an overlay table during a
  graduation — and it was found by a player, not by us.

1477: And the worst one, because it survived every check the project has: the `input-arms` merge
committed **four conflict markers to `main`**, in `README.md`, `docs/plan.md` and twice in
`docs/status.html`. Both sides of every hunk were **textually identical** — git had raised the
conflict on surrounding context — so the resolution was a no-op and nothing read wrong.
`figures.js` went on rewriting the marked number inside *both* halves and reporting success,
which is exactly what it should do and exactly why it saw nothing. Four of the project's five
checks looked at files that happened not to be hit. It was found by eye in a `git diff` run for
an unrelated reason.

`crates/l2-testkit/tests/conflict_markers.rs` closes that one. The general lesson does not close:
1487: **a merge conflict is a question about intent, and the resolution is an edit like any other
edit.** Resolve by reading both sides and writing what you mean. `--ours` and `--theirs` are
answers to a question nobody asked, and a union is a guess that the two sides are additive.

## A check that names a specific object is only as good as the name

`our_own_buttons_do_not_sit_on_anything_the_painter_drew` was written to stop us placing a widget
on top of one of the original's. It worked. It was also **asserting against the wrong rectangle**:
it named `OK` — the corner picture — while our CANCEL at (264, 446) was sitting squarely on
record 0 of `g_splitWidgets`, the army-division screen's **confirm tick**. Aiming at the bottom of
the game's split got our cancel instead.

The check was accurate about the rectangle it named. **The name was not checked by anything**, and
there is nothing in a string literal that can be wrong in a way a compiler or a test can see.

> **A test that names its subject in prose has moved the assertion out of the tree and into the
> name.**

Where the subject can be derived — from the widget table, from the file, from the enumeration —
derive it, and let the check quantify over everything
must be named, the name deserves the same scepticism as a number: ask what would be
different if it were wrong, and if the answer is *"nothing visible"*, that is the finding.

## A hand-staged fixture can manufacture a finding, and it will be believed

A siege test staged its besieger by hand: units placed, engines ordered, state set. It was a
reasonable fixture and it was **wrong about ordering** — and because every siege test in the
1514: workspace staged its besieger the same way, none of them travelled the real road. One of them
wrote *"the AI never orders siege engines, so an AI besieging a level-3 castle can never assault
at all"* into its module header.

That statement then went into a report, from the report into a brief, and from that brief into
another agent's brief. It is false. The AI's path is `Unit_ReachCastleBuilding` →
`Army_BeginSiege` → `Siege_Link` → `Siege_Prepare`, all four implemented and faithful all along.
What was true — *"no AI calls `order_engine`"* — is a statement about **one function**, and
`order_engine` is the siege screen's + and − buttons, which the original's AI does not press
either.

1525: Two things to take from it.

**A fixture that constructs the state a feature is supposed to produce cannot tell you whether
the feature produces it.** The fix is a test that walks the road: an army moved onto a castle
1529: tile, and the order read out of what came back. That test did not exist for any siege in the
workspace, so the hole was subsystem-wide, not one test's oversight.

**And name the function.** This is the same shape as C71 — a true statement about one branch,
promoted to a statement about a subsystem, in prose, between agents, where no check in the tree
can reach it. The whole defence is one word. *"No AI calls `order_engine`"* cannot be promoted by
accident; *"the AI never orders siege engines"* already has been.

## Citing an oracle is not reading it

`CLAUDE.md` promotes the shipped `Readme.txt` to a first-class oracle: it is the v1.03 patch's
rules errata, it post-dates the manual, and it wins wherever they disagree. That promotion was
right and it has paid for itself repeatedly.

It did not stop this.

`Readme.txt` says an army is destroyed when it has *"less than 50 men **after** retreating."*
**This project has quoted that sentence twice** — in two different documents, both times as
supporting evidence — while implementing the test on the total **before** the halving. The
citation was accurate.
the sentence for its order of operations; they were reading it for the number 50, which they
already had.

That is the failure, and it is not carelessness:

> **A citation is retrieved to support a claim you already hold, so it is read for the part that
> supports it and skimmed for the rest.** The parts you did not need are exactly the parts that
> would have corrected you.

The corrected reading — `Army_WithdrawCasualties` (`0x004AD8CC`) halves every line *above* the
`menTotal < 50` branch, so an 80-man army becomes 40 and is destroyed — was found in the binary,
by an agent adding a missing writer, and only then recognised in the sentence that had been on
file all along. `docs/decisions.md` C71.

**The practice.** When an oracle is cited for a fact, **quote the whole sentence into the code or
the document, and then read the quoted text once more against what is being written** — not
against the claim it was fetched for. On this project the oracles are short: a `Readme.txt`
paragraph, an `L2.eng` string, a decompiled function of forty lines.
reading a clause of one.

1569: And the sharper version, for a `Readme.txt` line especially: **the words "after", "before",
"each", "total" and "remaining" are where the mechanics live.** Those are the words a reader
skims when they are looking for a number.

## A test that asserts the defect is not an accidental pass — it is worse

This document catalogues thirteen checks that passed for the wrong reason. Every one of them
was a check that **could not fail**: a pixel count measuring the artwork, a marker rule
agreeing with today's data, a geometry check comparing two of our own constants. The remedy
was always to make the check capable of failing.

This one is different in kind and needs saying separately, because the remedy does not apply:

> **A test that asserts the defect is a check that would have failed on the *correct* code.**
> It does not pass for the wrong reason. It passes for exactly the right reason, against the
> wrong claim.

The case. `Game::set_ration` wrote `ration_wanted` and returned, where
`Ration_IncreaseCounty` (`0x0043A23F`) calls `Ration_Apply` and repaints — so in the original
the *achieved* level moves the instant you press the arrow. And
`orders_are_clamped_to_the_ranges_the_rules_have` contained:

```rust
let achieved = g.kingdom.counties[1].ration_achieved;
g.set_ration(1, -3);
assert_eq!(
    g.kingdom.counties[1].ration_achieved, achieved,
    "what the player asks for (+0x15E) is not what the county managed to feed (+0x15D)"
);
```

The *sentence* is true — the two fields are different things. The *assertion* is
false, and it was derived from the code, not from the binary: someone observed that our
setter left `ration_achieved` alone, recognised a real distinction that explains it, and wrote
the observation down as a requirement. Fixing the code turned the test red, which is how it
was found.

**Nothing in this file would have caught it.** Ablation cannot: deleting the line the assertion
is about makes it fail, correctly, because the assertion *is* about live code. Two artefacts
1608: that must agree cannot: the code and the test agreed perfectly. The only thing that finds it is
the thing that found it — implementing what the binary does and watching a test object.

### What it means for practice

**A test written by reading our own code is a description, not a specification.** It records
what we do. That is worth something — it catches regressions — but it must not be written in
the voice of a rule, because the next reader cannot tell the two apart, and a red test is
normally evidence that the *change* is wrong.

So, when a fix turns an old test red:

1. **Read what the old test claimed, against the binary, before assuming the fix is wrong.**
   1621: Here the rule cited in the message (`+0x15E` is not `+0x15D`) was true and the assertion
   built on it was not — a correct premise carried into a wrong requirement, which is the
   shape that survives review.
2. **If the old assertion was a description, replace it with the claim it was reaching for**
   that does not depend on the bug: ask for triple rations with an empty larder and require
   `wanted == 5, achieved == 0`.

1628: ### And the vacuous pass caught in the act, in the same test

That replacement failed on its first run for an unrelated reason worth recording: `two_counties()`
builds a county with **no population**, and a county with nobody in it is fed at *Triple*
trivially, because the requirement is zero. The assertion `achieved == 0` read `5`.

Had the numbers happened to line up, it would have passed while measuring nothing — the same
family as the sweep test three functions away whose first draft never fired its search because
the county's herd was not being eaten. **Both were caught by running the test and reading the
1637: number, not the verdict**,
keeps paying: a test that passes on the first attempt against a state you did not deliberately
construct deserves thirty seconds of *why*.

## The correction that identifies a class must enumerate the class

1643: Nearly everything above is about a check that fails to fire. This one is about a *fix* that
fires exactly once and looks complete.

> **Naming a category and fixing one member of it is the most expensive kind of half-finished
> work, because the name makes it look finished.**

The case. A player reported the ration panel's slider as *"moves but is inoperable"*. Reading
the binary produced a good correction and a genuine category: **a control in this game
recomputes and repaints — `Ration_SetSplit` runs the food pass on the spot, searches,
1652: reallocates the county twice and calls `Panel_Ration()` — so a setter that only sets is not
1653: the control.** The slider was fixed, the category was written down, and the work read as done.

An hour later the same player reported the **tax** panel, three feet away, with two symptoms
in one sentence. Same widget-table shape, same missing call, same panel group.
`Tax_IncreaseCounty` is `taxRate++`, `Tax_RecomputePreview`, `Panel_Tax()`, and ours wrote the
field and returned.

**The correction named the class and fixed one member.** Nothing in it was wrong; what was
missing is the step after — *now list the others*. Five setters, four lines of grep:

| ours | the original | state when the table was written |
|---|---|---|
| `set_ration_split` | `Ration_SetSplit` | fixed |
| `set_tax_rate` | `Tax_IncreaseCounty` | broken, reported by a player |
| `set_ration` | `Ration_IncreaseCounty` | **not read** |
| `set_industry_share` | `FUN_00439122` | already correct |
| `toggle_industry` | `Industry_ToggleFromMap` | already correct |

The third row was filed **`open`, not "probably fine"**, and reading it the next morning took
ten minutes and found the same defect a third time — on the same panel as the first. **Nobody
reported that one.** The enumeration got there
first, and that is the entire argument: the third instance was the cheapest of the three to
find and would have been the most expensive to ship, since by then three copies of one
omission would have looked like a fact about our architecture.

### Two habits this leaves

**File the unread members `open`.** An unread member of an enumerated class is a *known*
unknown, and every expensive thing in this document is the other kind. `open` costs a row in a
table and buys the difference between "we checked" and "nobody has looked", which is the only
thing the next reader needs from you.

**Write down the members you looked at and cleared.** The ration path has `ration::preview`
and `ration::apply` — the same name as the original's single `Ration_Apply`, opposite
behaviour on the store, and reaching for the wrong one would have had a drag eat the county's
herd a hundred times over. The tax path was checked for the same trap and has none. *Looking
and finding nothing is the half of a check that normally goes unwritten*, and an empty result
recorded is worth almost as much as a finding, because it stops the next agent spending the
same twenty minutes.

### Where the boundary is

This is not an instruction to chase every neighbour of every fix. The trigger is narrow and
mechanical: **if your correction contains a sentence of the form "X in this game always does
Y", then before you finish, list the Xs.** A class small enough to name is nearly always small
enough to enumerate — there were five — and if it is not, saying so in the correction is
itself the finding.

## Prior art first

Before commissioning a reverse-engineering task, spend five minutes searching for existing
documentation or tools. This project lost real effort re-deriving a format that was already
published. Make it the first step in the brief.
