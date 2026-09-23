---
name: oracle-compare
description: Compare your own lords2 work against the original Lords2.exe before you commit and before you report. Use whenever you write or change a behaviour (rule, guard, constant, order, draw position, string source, sound, input arm). It is the self-check; the oracle-check agent is the reviewer that reads your branch afterwards.
---

# Compare against the oracle

The binary is right; our code, docs and tests are claims (CLAUDE.md, *The idea*). Do this
**while building**, not after: a behaviour you cannot tie to a line of decompilation is
not done. Budget: about 3 tool calls per behaviour.

## 1. Before writing: find the original's function

```
node tools/oracle/dossier.js <0xADDR|Name>      # always first: file, callers, globals, strings, sounds
node tools/oracle/xref.js callers|calls|touches <name>
node tools/oracle/anchor.js strings --group <n> # which function draws L2.eng group n
node tools/oracle/widgets.js / kinds.js         # input tables (24-byte records, kind at +0x0F)
node tools/oracle/sounds.js | netcmds.js <op> | fields.js | logstrings.js --grep <text>
node tools/oracle/reaches.js <addr>             # is it live at all?
```

Then Read the body in `tools/oracle/decomp/<segment>.c` **with offset and limit**, never a
whole file. Names: `docs/symbols.md`; unverified names: `docs/name-leads.json`.
Deliberate departures: `grep -n 'C<n>' docs/decisions.md`; kept defects: `docs/bugs.md`.
No function found → that is the finding. Stop and report it; never invent (rule 5).

## 2. While writing: carry the evidence

Beside each behaviour, a comment with the function, its address, and the decomp line or
`+0x` offset (rule 7 shape). A deliberate difference is marked `[D]` with its reason or a
C-number. Strings come from the screen's `L2.eng` group (rule 6), not literals.

## 3. Before commit: the three questions, per behaviour

Put the Rust line beside the C line and answer:

1. **Order** — same sequence of operations? (halve *then* test, draw *then* hit-test)
2. **Guards** — every `if` the original tests, and none it does not?
3. **Constants** — offsets, counts, thresholds, table indices, coordinates, identical?

Read the whole C statement, not the part that holds the number you came for: "each",
"total", "remaining", "after" are where the mechanics live (agents.md *Citing an oracle is
not reading it*).

## 4. Prove it with a test that can fail

Assert the original's value, then ablate the line (agents.md *Ablate the line*): break the
behaviour, see the test fail, restore. Run `powershell -File tools/run/changed.ps1`, not the
full suite. Where an install-gated fixture or the live process can settle it,
`docs/oracle-requests.md` lists the cheap reads.

## 5. Report

Per behaviour, one line: `file:line — FUNC 0xADDR — match | [D] reason | unverified: why`.
Label every claim `[V]` verified (read in the decomp or measured) or `[I]` inferred. The
reviewer (`oracle-check`) will re-read your branch; anything you marked `[V]` that it finds
wrong costs a fix round.
