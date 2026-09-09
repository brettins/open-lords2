# Environment and commands

Everything here has cost real time at least once.

## Paths

| What | Where |
|------|-------|
| Repo | `E:\dev\lords2` |
| Game (GOG, Windows build) | `F:\games\Lords of the Realm II` — **read only** |
| Game (older DOS install) | `F:\games\LORDS2` — **read only**, useful for diffing |
| **Test fixtures** | `E:\dev\lords2-fixtures` — preserved saves, **outside every install** |
| Ghidra | `E:\dev\tools\ghidra_12.1.3_PUBLIC` |
| Ghidra projects | `E:\dev\ghidra-projects` |
| JDK 21 | `C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot` |
| Rust | `~/.cargo/bin` (not on PATH by default) |

## The test suite's three inputs

| variable | default | holds |
|---|---|---|
| `LORDS2_DIR` | `F:\games\Lords of the Realm II` | the install: `Lords2.exe`, the art, `USER.SKR`, `L2_maps.dat` |
| `LORDS2_FIXTURES` | `E:\dev\lords2-fixtures` | preserved `.sav` files that nothing rewrites |
| `LORDS2_DOS_DIR` | `F:\games\LORDS2` | the older DOS install, for the two tests that diff against it |

All three are resolved in **one place**, `crates/l2-testkit`, and a test in that
crate fails if a game directory is spelled out anywhere else. Thirteen files used
to carry their own copy of the install path.

### Fixtures are named, not found

**`lastturn.sav` inside a game install is the rolling autosave.** The game
rewrites it every turn a human plays, and a clean GOG install ships **no saves at
all** — verified by diffing a pristine copy of the install against a played one,
where exactly six files differ and five of them are saves. Nine tests treated one
particular `lastturn.sav` as a fixed fixture, called it "the shipped save", and
went red the first time somebody played for ten minutes.

So a fixture is a **file name plus a fingerprint**, and lives outside every
install:

| fixture | file | what it is |
|---|---|---|
| England turn one | `england-turn1.sav` | the England map, turn 1, Winter 1268. Fourteen counties; five owned at indices 1, 4, 8, 11, 13, one realm each; `g_localPlayer` 1 |
| the battle triple | `battle-before.sav`, `battle-during.sav`, `battle-after.sav` | one battle caught at three moments: 178 men at (33, 17); a defender in slot 6; both armies gone |
| the turn pair | `old_turn.sav`, `safeturn.sav` | earlier turns of the same game — a multi-turn economy |

`l2_testkit::england_turn1()` returns one of three states and they are kept
distinct on purpose, because conflating the last two is what hid the breakage:

* **ready** — the file is there and the fingerprint matches;
* **absent** — nothing is configured; the test *skips*, printing `SKIP <name>: <why>`;
* **wrong game** — something is there and it is a different saved game; the test
  **fails**, naming the fixture and saying which clause failed.

There is deliberately **no fall back to the install's `lastturn.sav`.** Three
directories on this machine hold a file of that name and they are three
different games.

**What the fingerprint deliberately excludes.** The realm→county assignment,
`g_weatherCounty`, which lord sits behind which realm, and which county the
person ends up on are all **rolled per game**. Comparing two independently
created England turn-one saves is what established that; asserting any of them
is what turned a regenerable fixture into an irreproducible one.

**Regenerating the England fixture.** Start a new England campaign in the
original game, let turn one begin, quit, and copy that install's `lastturn.sav`
to `%LORDS2_FIXTURES%\england-turn1.sav`. Never copy it *into* the repository:
`.gitignore` refuses `*.sav` and a test in `l2-testkit` fails if one appears in
the working tree.

## Commands

```bash
export PATH="$HOME/.cargo/bin:$PATH"

# What CI runs: no install, no fixtures. The install-gated tests skip and say
# so; crates/l2-testkit/tests/census.rs names every one of them and prints how
# many the current environment satisfies.
cargo test --workspace

# Everything, including the corpus and the fixture-gated suites.
LORDS2_DIR="F:\games\Lords of the Realm II" \
LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test --workspace

# What this run actually asserted, and what it silently did not.
cargo test -p l2-testkit --test census -- --nocapture
```

**Both of the first two must pass, and they are different suites.** That is the
whole point of the census: `cargo test --workspace` prints the same
`passed; 0 failed` line either way while asserting wildly different amounts, so
the number of gated tests is written down in
`crates/l2-testkit/tests/census.rs` and a new gate fails the build until it is
added there.

Ghidra headless. The `lords2` project is **already imported and analysed** — reuse it,
don't re-import:

```powershell
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot"
& "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat" `
    "E:\dev\ghidra-projects" lords2 -process Lords2.exe -noanalysis `
    -scriptPath "E:\dev\lords2\ghidra_scripts" `
    -postScript DecompileFunc.java 0040a21a
```

`DecompileFunc.java` takes any number of hex addresses as script arguments.

## Gotchas

- **Backslashes are eaten** by the shell layer inside `sed`, `awk` and inline `node -e`.
  A Windows path written as `F:\games\...` arrives as `F:games...`. Use forward slashes,
  or a placeholder character piped through `tr`.
- **Node needs forward slashes** on Windows: `F:/games/...`. It does not understand Git
  Bash's `/f/...` mapping and silently resolves it to `C:\f\...`.
- **Ghidra 12 has no Python here** — PyGhidra isn't installed and there's no Python 3 on
  the machine. Write Ghidra scripts in **Java**; the class name must match the filename.
- **One process per Ghidra project.** Parallel agents need separate project directories.
- **`strings` is not installed.** Use `grep -a`.
- **PowerShell is 5.1**: no `&&`, no ternary, no null-coalescing, and avoid `2>&1` on
  native executables.
- Heredocs with lots of quoting break easily; prefer writing files with the editor tools.

## Screen capture and input

`tools/screen.ps1` captures a window via `PrintWindow`, which works even when the window
is occluded — plain screen capture grabs whatever is physically on top instead.
`tools/input.ps1` sends clicks in client-relative coordinates. `tools/probe.ps1` reads
memory from a live process.

Note: taking a screenshot by spawning a process can steal focus, and the game crashes if
it is deactivated during startup. Don't capture during the first few seconds of a launch.

### Synthetic input needs window focus (verified)

`input.ps1 -Action key` uses `SendKeys`, which goes to whatever window is focused, so it
needs the target in front. `SetForegroundWindow` also **fails silently** from a background
process, so a script that assumes it worked will type into the wrong window.

`-Action postkey` posts `WM_KEYDOWN`/`WM_KEYUP` straight to the window handle with a
correctly formed `lParam` (repeat count, scan code in bits 16-23, extended flag in bit 24 -
`lParam = 0` is silently dropped). In principle that bypasses focus. **In practice it does
not work on our `winit` viewer**: minimising the window so it cannot be focused, then
posting keys, produced no key events at all, while the identical sequence worked the moment
the window had focus.

Measured, not assumed - and the measurement went the opposite way to the assumption twice
before it was checked properly. If a test needs the window unfocused, minimise it and read
the title with `GetWindowTextW` on the handle; `Process.MainWindowTitle` returns empty for
a minimised window and will look like a failure that isn't one.

The `postkey` path is kept because the original game is a plain Win32/DirectDraw app rather
than a `winit` one and may well accept posted messages. That is untested.
