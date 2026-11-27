# Contributing to open-lords2

Thanks for looking. There are two ways to help, and the first needs no code at all.

## 1. Remember the game

The most valuable thing anyone can offer is **memory of playing Lords of the Realm II**.
[`docs/mechanics.md`](docs/mechanics.md) lists every mechanic and how sure we are of it;
it exists to be read by a player and contradicted. If something here plays differently
from the game you remember, or the original does something we don't, open an issue
(there is a template for mechanics). A save file or a screenshot from the original is
gold.

## 2. Work on the code

### Build and run

You need stable Rust ([rustup](https://rustup.rs)) and your own copy of the game.

```
cargo run --release -p l2-game -- "<your Lords of the Realm II folder>"
```

On Linux, install the audio and windowing headers first (Debian/Ubuntu names):
`libasound2-dev libxkbcommon-dev libwayland-dev libx11-dev pkg-config`.

### Tests

```
cargo test --workspace
```

That runs without the game, which is what CI does. Most of the interesting tests compare
against the original's files and skip themselves without them. To run those too, copy
[`paths.local.example`](paths.local.example) to `paths.local` (it is gitignored) and
point `LORDS2_DIR` at your install. `cargo test -p l2-testkit --test census -- --nocapture`
prints how many of the gated tests your setup satisfies.

### Ground rules

- **Never commit game files.** No `.pl8`, `.256`, `.smk`, `.wav`, extracted images or
  decompiled code. `.gitignore` blocks the common ones; please don't work around it.
- **The original is the authority.** `Lords2.exe` decides what the game does. When you
  implement or change a behaviour, cite the function in the binary it comes from
  (`docs/symbols.md` names them) next to the code. "Seemed right" is not enough;
  "couldn't find it in the binary" is a fine thing to write down.
- **The original's bugs stay.** Read [`docs/bugs.md`](docs/bugs.md) before "fixing" a
  rule that looks odd. Fixes are opt-in, never the default.
- **The simulation is deterministic.** Network play runs in lockstep, so game logic uses
  no floats where order matters, a seeded random generator, and no hash-order iteration.
  [`docs/netcode.md`](docs/netcode.md) has the details.
- **MIT, clean room.** The useful prior art for this game is GPL-3. Facts about file
  formats are free to use; that code is not. Don't copy or translate it.
- Keep pull requests small, with tests, and CI green. If you add tests, run
  `node tools/figures/figures.js` once: the README quotes the test count, and CI checks it.

### Where to read

| to… | read |
|---|---|
| learn what the game does, with real numbers | [`docs/rules.md`](docs/rules.md) |
| see what works and what doesn't | [`docs/status.html`](docs/status.html) |
| work on a file format | [`docs/formats/`](docs/formats/) |
| find a function in the original binary | [`docs/symbols.md`](docs/symbols.md) |
| touch battles, the economy, or armies | [`docs/battle.md`](docs/battle.md), [`docs/kingdom.md`](docs/kingdom.md), [`docs/armies.md`](docs/armies.md) |
| make or change a mod | [`docs/modding.md`](docs/modding.md) |
| set up the tooling (Ghidra, the oracle tools) | [`docs/environment.md`](docs/environment.md) |

The big documents are long; read the section you need, not the whole thing.

## How this project is developed

Much of this codebase was written by AI coding agents (Claude) working under the
maintainer's direction, checked against the original binary at every step. That
workflow leaves files you can ignore as a contributor:
[`CLAUDE.md`](CLAUDE.md) and `.claude/` (the agents' instructions),
[`docs/agents.md`](docs/agents.md), [`docs/work.json`](docs/work.json) (the work ledger)
and [`docs/decisions.md`](docs/decisions.md) (every design decision, by C-number). They
are kept in the open because they are the full record of *why* the code is the way it is:
grep `decisions.md` for a C-number you see in a comment.
