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

## Prior art first

Before commissioning a reverse-engineering task, spend five minutes searching for existing
documentation or tools. This project lost real effort re-deriving a format that was already
published. Make it the first step in the brief.
