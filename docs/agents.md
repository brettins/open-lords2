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

## Prior art first

Before commissioning a reverse-engineering task, spend five minutes searching for existing
documentation or tools. This project lost real effort re-deriving a format that was already
published. Make it the first step in the brief.
