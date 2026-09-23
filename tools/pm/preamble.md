# lords2 agent preamble (read once, first)

- You are in a git worktree of the lords2 checkout. First command:
  `git reset --hard main && git checkout -b {branch}`; check with
  `git branch --show-current` before every commit. Never touch the main checkout.
- Build output: `CARGO_TARGET_DIR=C:\Temp\l2-{slug}`. Game install and fixtures:
  `docs/environment.md` (the install is read-only).
- Test with `powershell -File tools/run/changed.ps1` (touched crates only). Never
  `cargo test --workspace`; the lead runs the suite at merge.
- Before reading any decompilation: `node tools/oracle/dossier.js <addr|name>`.
- Parity work: invoke the `oracle-compare` skill (`.claude/skills/oracle-compare/SKILL.md`)
  and follow it per behaviour. Netcode work: do NOT use the oracle; `docs/netcode.md` is the design.
- CLAUDE.md rules bind you, esp. 4 (label [V]/[I]), 5 (cite function+address), 6 (strings from
  L2.eng), 7 (short comments with evidence; files <500 lines src, <300 tests). Read big docs by
  section (`grep -n '^#'`).
- Oracle tools and `tools/oracle/decomp`/`out` are gitignored and live only in the main
  checkout: run them from there (`node <main>/tools/oracle/<tool>.js`) and read decomp there.
  Do not copy them.
- Every new behaviour gets a test that fails when you ablate the line (docs/agents.md *Ablate the line*).
- Commit your own paths only, small commits, message says what the player sees change.
  Do not edit docs/work.json, figures, symbols.md or status.html; the lead does.
- **The call budget is a hard stop: {budget} tool calls.** Count them. At 80% of it,
  stop starting new rows: finish, commit and report. Unfinished rows are a normal outcome.
  Batch shell work into one call; read files with offset/limit.
- A new docs/decisions.md entry uses a placeholder `CNEW-<slug>` (and a new bug `BNEW-<slug>`),
  cited the same way everywhere; the lead numbers it at merge. Never pick a number yourself.
- When you fix an oracle-check finding, set that record entry's verdict to `match` with a note
  "fixed: …" (verdicts are only match / mismatch / departure) and point `line` at the cited line.
- Never run `cargo fmt` on the workspace (it reformats ~930 unrelated files).
- If a row turns out not to be a defect, or cannot be settled from the decomp, stop on it and say so.
- Delete `$CARGO_TARGET_DIR` when done.
- Report (≤25 lines): per row: done / not done + why; per behaviour `file:line — FUNC 0xADDR — [V]/[I]`;
  tests added; what you left alone. No narration.
