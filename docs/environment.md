# Environment and commands

Everything here has cost real time at least once.

## Paths

| What | Where |
|------|-------|
| Repo | `E:\dev\lords2` |
| Game (GOG, Windows build) | `F:\games\Lords of the Realm II` — **read only** |
| Game (older DOS install) | `F:\games\LORDS2` — **read only**, useful for diffing |
| **Test fixtures** | `E:\dev\lords2-fixtures` — preserved saves, **outside every install** |
| **Our own saved games** | `%APPDATA%\open-lords2\saves` — §"Where our saves go" below |
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

## Where our saves go

**`%APPDATA%\open-lords2\saves`**, and `%LORDS2_SAVES%` overrides it. On a
non-Windows machine it is `$XDG_DATA_HOME/open-lords2/saves`, falling back to
`$HOME/.local/share/open-lords2/saves`. The directory is created on the first
*write* and never on a read or a listing, so opening the load screen on a
machine that has never saved touches nothing.

| variable | default | holds |
|---|---|---|
| `LORDS2_SAVES` | `%APPDATA%\open-lords2\saves` | **our** saved games, `.l2sav` |

It is resolved in one place, `l2_game::saves::dir`, and nothing else in the
workspace names a save directory.

**Why not the three obvious places.**

* **Not inside the game install.** `CLAUDE.md` rule 2 — read only — and it is
  also where the original keeps `lastturn.sav`, whose fixture identity this
  document has already watched a program destroy once.
* **Not inside the repository.** `.gitignore` refuses `*.sav` and `*.l2sav`, and
  `crates/l2-testkit/tests/census.rs` fails if either appears in the tree. A
  save that lands beside the source is a save somebody commits.
* **Not beside the executable.** That works for a portable build and fails for
  an installed one: `%PROGRAMFILES%` is not writable by the user who runs the
  game, and the failure arrives at the worst moment, when somebody presses save.

**The extension is `.l2sav`, not `.sav`.** The original's `.sav` is an
unversioned memory dump we read as an *oracle* (`l2_formats::save`, whose schema
comes out of the user's own `Lords2.exe`). Ours is a versioned format of our own
(`l2_game::save`, `l2_kingdom::save`) that we write and read. A directory
listing that cannot tell them apart is one somebody eventually confuses, and
`docs/decisions.md` D11 records the whole decision.

**We do not write the original's format**, and reading it is a separate job. A
save the original could open is not on any plan yet; what exists is a save
*we* can open, which is what "quit and resume" needs.

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

### Building while the game is open

**Launch the game with `tools\run\play.cmd`, not with `target\debug\l2-game.exe`.**
Windows will not let a build replace a running executable, so a game started from the
build output turns every build in the workspace red — `cargo test --workspace` included,
because it builds the same binary. It is cargo's last step that fails, not the linker:

```
error: failed to remove file `…\target\debug\l2-game.exe`
  Access is denied. (os error 5)
```

```powershell
tools\run\play.cmd "F:\games\Lords of the Realm II"              # debug build, then play
tools\run\play.cmd -Release "F:\games\Lords of the Realm II"     # release build, then play
tools\run\play.cmd -NoBuild "F:\games\Lords of the Realm II"     # play what is there, no build
tools\run\play.cmd "F:\games\Lords of the Realm II" --no-sound   # any l2-game flag passes through
```

It makes two promises, and both were driven end to end rather than read off the script
(`docs/decisions.md` CNEW-launcher-never-launched):

* **A build succeeds while the game is open.** It runs a *copy* — `l2-game-live.exe`, or
  `l2-game-live-1.exe` and so on for a second instance — so the build output is never
  locked. With a game held open by it, `cargo build -p l2-game` and
  `cargo test -p l2-game` both went through.
* **It always runs the newest build.** It builds first, and **a failed build launches
  nothing**: it prints `build failed - not launching.` and exits with cargo's code, rather
  than falling back to the last binary that compiled. `-NoBuild` is the only way to run
  something older, and you have to ask for it.

A copy still in use survives the sweep; the rest are deleted on the next launch. One
PowerShell trap: a single-dash game flag that is a prefix of `-Release` or `-NoBuild` is
taken by the launcher (`-n` becomes `-NoBuild`). None of `l2-game`'s own flags is.

**Pointing a desktop shortcut at it.** Change two fields in the shortcut's Properties:

| field | value |
|---|---|
| Target | `E:\dev\lords2\tools\run\play.cmd "F:\games\Lords of the Realm II"` |
| Start in | `E:\dev\lords2` |

A console window shows the build, then the game opens. `Start in` is not load-bearing —
the script finds the repository from its own location, and the same shortcut run from
`C:\Windows\Temp` did the same thing — but it costs nothing. Verified by building a
`.lnk` outside the desktop and running exactly the Target, arguments and Start in it
stored, with a game directory containing spaces; the game's own error named the whole
path intact. **Not verified:** a double-click, because that opens a window.

**A shortcut already aimed at `target\debug\l2-game.exe` is unaffected.** The binary is
still built at that path, under that name, by the same command — which is the reason
`build.rs` renames the *stale* file rather than versioning the new one. It keeps working;
it just locks the build output again, so builds collide with a game started from it as
they always did.

**The two fallbacks, for somebody who runs the exe directly.**

* `crates/l2-game/build.rs` moves a locked `l2-game.exe` aside to
  `l2-game.old-<ms>.exe` and lets the build through — **but only on a build that follows a
  commit, checkout, rebase or `git add`**, because the script watches only git's `HEAD`
  and `index`. An ordinary edit does not rerun it, and that build still fails. Measured,
  on a clean fingerprint. Moved copies are deleted on a later rerun, once nothing is
  running them.
* `L2_ALWAYS_UNLOCK=1` makes that move happen before every build, **and every build then
  recompiles `l2-game`: 2.6–3.2s against 0.17–0.21s for a no-op**, measured. That is
  paid by every build in the workspace while the variable is set, which is why it is
  opt-in.

```bash
L2_ALWAYS_UNLOCK=1 cargo build -p l2-game
```

**Measuring the default straight after using the switch measures the switch.** A
switched build leaves cargo rerunning the script until one plain build re-emits a clean
fingerprint.

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
