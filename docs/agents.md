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

Give every agent its own output files. Two agents editing one document will clobber each
other, and neither will notice.

## Ghidra

**Only one process may hold a Ghidra project at a time.** Concurrent agents must use
separate project directories under `E:\dev\ghidra-projects` — e.g. `lords2` for the game
binary and `mapl2` for the shipped map editor. Say so explicitly in the agent's brief;
this is not obvious and the failure mode is a lock error mid-run.

Copy `ghidra_scripts/DecompileFunc.java` into a per-agent script directory rather than
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

The form that actually holds names the paths on the **commit**:

```bash
git commit -F msg.txt -- crates/l2-mods docs/modding.md   # yes: only these, whatever the index holds
git add crates/ && git commit -F msg.txt                  # no: commits the whole index
git add -A                                                 # never
```

`git commit -- <paths>` bypasses the index for those paths and commits exactly what you
name. Check `git status --short` first and confirm every path is yours; when several agents
are live, that check is necessary and not sufficient, and the `--` form is what closes the
gap.

**One catch, and it bites on the first commit of anything new.** `git commit -- <paths>`
only accepts paths git already knows, so a brand-new file fails with
`pathspec … did not match any file(s) known to git`. A new file must be `git add`-ed first,
and *then* committed with the `--` form:

```bash
git add tools/oracle/xref.js
git commit -F msg.txt -- tools/oracle/xref.js    # add makes it known, -- keeps it alone
```

The `git add` is safe here because it names one path; it is the bare `git commit` afterwards
that would sweep the index, and the `--` prevents exactly that.

**And a second catch, sharper than the first: `--` is wrong for removals.**
`git commit -- <paths>` commits the **working tree** at those paths and ignores the index.
That is exactly what makes it safe for edits — and it silently *reverses* a deletion you
staged with `git rm --cached`, because the file is still on disk. Untracking 1,891 build
artefacts this way put all 1,891 straight back, and the commit looked like it had worked.

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
it or simply abandoned it.

```powershell
Stop-Process -Name Lords2,l2-view,dxwnd -Force -ErrorAction SilentlyContinue
```

Run that at the end of the task whether it succeeded or not. Put the requirement in the
brief — an agent told it may run the game will not infer that it should also close it.

Prefer short focused sessions with the game over keeping it open across a long
investigation, and if a task genuinely needs it open for a long stretch, say so in the
report so the cost is visible.

## Never drive the real mouse or keyboard to test our engine

**An agent testing our engine does not touch the OS input queue.** Synthetic input moves
the cursor and steals focus on a machine somebody is sitting at — this was found the way
these things are always found, by the person whose mouse jumped.

There is no reason to reach for it, because **everything our engine does is reachable as a
value**. `Event::Click { x, y }`, `Event::Pointer`, `Event::KeyDown`, handed to
`Machine::handle` with a `Ctx`; `crates/l2-game/tests/screens.rs` and `tests/machine.rs`
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
reasoning behind reading a struct definition rather than enumerating fields by hand, and
behind a check that runs on every push rather than a rule an agent is asked to remember:
**the mechanism with fewer dependencies on the world is usually also the more accurate
one**, and where the two pull apart it is worth noticing why.

## Concurrent agents: unique scratch paths, and count before and after

Two agents picked the same scratchpad filename on the same day, and one spliced the other's
half-written symbol list into `docs/symbols.json` by mistake. It was caught, reverted and
redone. What caught it was **a count check** — read the number of entries before the edit and
after it, and confirm the difference is the number you meant to add.

Two rules follow, and they are cheap:

* **Every agent uses scratch filenames unique to itself.** Put the agent's own id in the
  path. A shared temp directory with a predictable name — `out.json`, `syms.json`,
  `tmp.txt` — is a collision waiting for the day two agents run at once, and that day is
  now normal here rather than rare.
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
CI and fails on a duplicate number, naming both headings and the next free one; it also
fails on a citation of a correction that does not exist, which is the other half of the same
problem and the one that went unnoticed for weeks. Run it before you finish and you will not
hand the integrator a collision.

It is still not worth serialising the log, and the integrator can renumber safely. What
makes that cheap rather than archaeological is one line:

> **Say in your report which correction number you took**, and grep the tree for
> cross-references to it before you finish — `symbols.json` comments, Rust doc comments and
> other documents all cite corrections by number.

The integrator then renumbers the later arrival deterministically instead of discovering the
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
The first proposal was to make citations self-describing — write `C34 (secession-tie)`, so a
dragged citation carries its old slug against its new number and fails immediately. It was
refuted by measuring rather than by arguing:

* **80 of 261 citations are shapes that would not survive it.** Lists especially:
  *"C10, C12, C17 and C20 are four instances of one mechanism"* becomes unreadable when every
  number carries a parenthetical, and these documents are written to be read.
* **Half the slugs derived from headings say nothing** — `files-fail`, `names-doing`,
  `reading-five`.
* And the detail that ended it: **two slugs embed a different correction's number.** C16's
  heading opens by referring to C14 and C20's to C12, so the mechanical slugs come out
  `c14-second` and `c12-second`. **A disambiguation scheme derived from the thing being
  disambiguated can inherit its ambiguity**, and a slug that names the wrong correction is
  worse than no slug at all.

The lockfile costs 10 KB, has no migration, changes nothing about how a citation is written,
and flagged the real historical failure with zero false positives across a merge that moved
seven citations and drifted every line number in the tree. That is the general lesson too:
**when two mechanisms are proposed, measure them against a failure that actually happened**
rather than reasoning about which is more elegant.

**An honest note about the paragraph above, because the record is worth more than the rule
looking effective.** The protocol in this section was written after four collisions and then
did not prevent the fifth — but it was never actually tested: the branch that collided had
been cut *before* the protocol landed, so the agent never read it. So there is no evidence
either way about whether writing it down works, and there is now no need to find out.
Documented process depends on an agent having read the document, which a long-running branch
by construction may not have; a check in CI does not. That is the general lesson, and it is
worth applying to the next process rule this file gains: **prefer the version a machine
enforces over the version an agent is asked to remember**, and if you write the second, plan
to replace it with the first.

## A test that drives the picture from the wrong field passes for ever

The village screen had **eleven tests and not one asked what the county's own record
said.** Every one drove the picture from the county's *labour* fields, which the scenario
importer fills. Not one drove it from the *industry* fields, which it did not — four
24-byte records per county were skipped wholesale, so `has_resource` and all four enable
switches came from `County::new()`'s defaults, and **every map toggle sat in the opposite
position to the one the player saw.** Eleven passing tests, and a struct field that no
importer had ever written.

That is C30 from the drawing side rather than the save side, and the pair states the rule
better than either alone: **a field is only tested if something a test reads was written by
something the game runs.** A test that populates the state it then asserts on is checking
its own fixture. C30's four fields were absent from the *encoding*; these were absent from
the *import*; in both cases the suite was green and the field was fiction.

Two practical consequences:

* **When you add a field to a record, ask what writes it in a real game** — an importer, a
  season pass, a click — and make at least one test travel that road. If nothing writes it
  yet, that is worth knowing and worth saying at the field.
* **Prefer a fixture the game produced to one a test built.** `l2_testkit`'s gated
  fixtures exist for this; a hand-built `Kingdom` carries whatever `new()` gives it, and
  `new()` agrees with every wrong reading equally.

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
   recentres* — and the conclusion drawn was that the fix asked for was too big: the original
   selects, it just does not open anything.
3. That reading was written up as C61, **C58 was edited in place to apologise for its claim that
   `Map_Click` has no county-selection arm**, `screens/map.rs`'s module header was rewritten around
   it, and two tests were rewritten to assert it. All of it passed.
4. It is the **prologue of the industry branch**, guarded by tile flag `0x80` *and* by the county
   being the local player's. `Map_Click` has no such arm; its three writes to `g_selectedCounty`
   are in the village, industry and merchant branches, exactly as C58 said. C58 was correct.

**Why this is worse than a wrong tool.** A tool's output is treated as a lead. A correction is
treated as settled — the whole point of the log is that it outranks prose written earlier, and
C58's flat statement is precisely what stopped anyone re-reading `Map_Click` for weeks. A wrong
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
*smaller* than the fix that was asked for, and the fix that was asked for was smaller than the one
the draft proposed. Every time this project has actually read the binary rather than reasoned from
an absence, the answer has been less work than the guess.

## C-numbers: the protocol assumes a serial writer, and we have eight

Six collisions now. The first four produced the protocol — *read the file, take the highest
number, add one* — and the fifth happened anyway. The sixth was today, three ways at once:
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
  collision *mechanical* rather than textual — two files claiming the same number is trivially detectable —
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

At merge, `node tools/decisions/corrections.js --assign` walks the branch's headings in the order
they appear in `decisions.md`, maps each distinct placeholder to the next free number, rewrites
**every occurrence across the whole tree** — headings, prose, `symbols.json` comments, Rust doc
comments, test names — and relocks the citations. One command, no chasing.

**And the check that makes it structural rather than a habit: `--check` fails if any `CNEW`
survives anywhere on `main`.** That is what stops the placeholder from being merged unassigned,
and it is the whole enforcement. It goes red on ordinary work, in CI, on the commit that would
have introduced the problem — which is the property *"read the highest number first"* never had.

What it costs: an agent's own branch reads `CNEW-hover` instead of `C61` while the work is in
flight. That is a branch, and it is correct that a number which has not been allocated does not
appear.

What it does not fix, said plainly: **two agents writing corrections about the same thing.** That
is a content collision, not a numbering one, and no tool resolves it — it is the coordinator
knowing what is in flight. Today's three were three genuinely different subjects that happened to
want the same integer, which is the case this removes entirely.

### Why this is written here and not just done

Two renumbers were done by hand today. Both were clean because no file cited both colliding
entries — the citation lockfile caught one drag on `symbols.md:307` and confirmed the rest — but
that was luck about which documents the branches touched, not a property of the method. The next
collision will be between two branches that both cite the same file, and the hand method's
failure mode there is a citation quietly renumbered to point at the wrong correction: a wrong
pointer into the log the project trusts most, which is the failure mode recorded above under
*The correction log can be wrong*.

## A tool that degrades silently is worse the more people use it

`tools/oracle/decompile-all.ps1` **exits non-zero** when `ApplySymbols` or
`ApplyRecords` drops an entry, and `tools/symbols/symbols_md.js --check` refuses four
shapes of `signature` field that cause it. Both exist because of the same two incidents,
two days apart:

    void __cdecl Setup_SetOption(int, int)   a calling convention, which Ghidra's
                                             C parser rejects outright
    int g_goodsStall[14][5]                  a data table filed under "functions"

**Each was dropped on every rebuild, and the pipeline reported success both times.** The
name never reached the corpus; `ApplySymbols` printed one line among a hundred; an
integrator happened to read it. That is not a control.

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

### What was proposed, and the two halves of it that do not survive

The proposal was: a per-screen inventory of the original's input arms, and a census that goes red
when a screen graduates out of the shell table without one.

**The inventory is right.** `Screen_FrameInput`'s `0x02` arm running six sidebar guards was found
by enumerating rather than by reacting to a report, and enumerating three screen groups is what
produced the 43% at all. Nothing else this project has tried finds an arm nobody asked about.

**The trigger is wrong, for a measurable reason.** "Red when a screen graduates" fires once per
screen, at graduation. Every miss found today is on a screen that graduated weeks ago; the check
would have caught **none of them**. Worse, it can only ever go red once — after the file exists it
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

The three checks that have actually worked here — `symbols_md.js`, `figures.js`, `corrections.js`
— all have the same shape, and it is not "a file must exist". It is **two artefacts maintained by
different work, that must agree.** A number in a document versus a number derived from the tree. A
citation versus the heading it names. That shape goes red on ordinary work, in both directions,
which is the whole point.

**The schema is being authored by the battlefield agent, not here.** It has ~50 records
blocked waiting on one and is the file's largest single contributor, which makes it the right
author; this section is the requirement, not the design. Three constraints are non-negotiable
and they are the reason the file exists at all: set equality in both directions between
`// arm: 0x…` markers and `reproduced` records; an **invention** must be representable and
countable rather than merely absent; and a **dead** arm must be distinguishable from a missing
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
   one-line marker — `// arm: 0x004A8E0B` or `// arm: ours` — and the check is *set equality*
   between the markers in the code and the `reproduced`/`ours` records in the file. This is the
   half that fires on ordinary work: a new handler with no marker fails, a marker with no record
   fails, a record claiming an arm nobody built fails, and a deleted handler fails. It is also the
   half that makes an invention **countable**, which is what a 1:1 goal actually needs — `ours`
   arms are a number that should go down.
4. **A per-screen percentage, printed by the census.** Not a gate: a number in `status.html` and
   the README, maintained by the script, that cannot drift because nobody types it. `figures.js`
   already does exactly this for other counts, and the precedent there is *"a number that cannot
   drift beats a number that is checked"*.

The falsification condition, since a check that cannot go red is a rule: **check 3 fails if I add a
`RightClick` handler to `divide.rs` without an entry, and fails if I delete
`MapScreen::update_hover_path` without one.** Both are one-line experiments and both should be run
before this is believed.

### What it costs, honestly

Enumerating three screen groups took three agents about half an hour of wall time each and
produced ~185 rows, most of which are verdicts rather than research. Marking up the existing
handlers is the larger job — perhaps a day — and it is unavoidable, because check 3 is worthless
until the markers exist and check 3 is the only one that fires on ordinary work.

**So do not start with all of it.** The three groups already enumerated are `map.rs`,
`village.rs`/`job.rs`, and `county.rs`/`divide.rs`/`shells.rs`, and their tables are in C61 and in
the three agent reports. Land those, run the two falsification experiments, and let the census go
red for a screen that has arms in the file and no markers in the code. The battlefield's 49 arms
are the argument for doing this *before* that screen is built rather than after — it is the only
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

So the totals in C61 are provisional until the corrected denominator lands, and the corrected
one goes into `tools/figures/figures.js` rather than into prose: it is about to be the headline
of `docs/plan.md` revision 5 and quoted widely, and 34 figures have already gone stale in four
documents by being typed. **A number that cannot drift beats a number that is checked.**

The general rule for the arms file: **every screen carries a reachability record beside its arm
records** — the addresses that write its id — and a screen with no writer is `dead`, which is
why `dead` has to be a distinguishable status rather than an absence. Two enumerations from
different directions are what has caught things all evening; one enumeration is a claim.

### One entry here that is not a failure

Everything else under this heading is something going wrong, which makes the file read as a
catalogue of carelessness. `docs/bugs.md` B65 is the good case and belongs beside them.

Two of the village's eight animation counters are stepped every frame and read by **nothing in
the entire binary**. One of them has 21 states and `villani1.pl8` happens to have 21 frames.
That is exactly the shape of a finding: a number that matches, a plausible story available for
free, and a decompiled function that would have looked like evidence for it. The agent recorded
the coincidence and **declined to build the story on it**, marking the counters dead and the
match unexplained.

That is rule 4 working — *a plausible story assembled from decompiler output is not a finding* —
and it is worth naming, because every other example in this file is what happens when the same
temptation wins.

### The rest of the pattern, recorded

Two other things fell out of the enumeration and belong here rather than in a correction:

- **Right-click is the gesture we systematically miss**, because "right click exits" was learned
  early and generalised. The original uses that button for four different verbs — exit, cancel a
  drag, clear a selection, re-centre a panel — and 11 of the 22 right-button arms found are either
  missing or wrong. One is *wrong* rather than absent: a right-click while carrying peasants leaves
  the village instead of cancelling the carry.
- **Two documented claims were falsified by the enumeration**, both of the "verified" tier:
  `Village_DoubleClick` is *not* the only reader of `g_mouseLeftDoubleClick` (`FUN_0043BF07` and
  `Hotspot_Test` read it too, and `crates/l2-game/src/input.rs` repeats the claim), and screen
  `0x12`'s arm has no right-button test at all, so `battle.rs`'s right-click-to-Decline is ours.
  Neither was found by looking for errors; both fell out of reading a screen exhaustively. That is
  the argument for enumeration in one sentence.

## Prior art first

Before commissioning a reverse-engineering task, spend five minutes searching for existing
documentation or tools. This project lost real effort re-deriving a format that was already
published. Make it the first step in the brief.
