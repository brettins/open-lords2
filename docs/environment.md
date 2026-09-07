# Environment and commands

Everything here has cost real time at least once.

## Paths

| What | Where |
|------|-------|
| Repo | `E:\dev\lords2` |
| Game (GOG, Windows build) | `F:\games\Lords of the Realm II` — **read only** |
| Game (older DOS install) | `F:\games\LORDS2` — **read only**, useful for diffing |
| Ghidra | `E:\dev\tools\ghidra_12.1.3_PUBLIC` |
| Ghidra projects | `E:\dev\ghidra-projects` |
| JDK 21 | `C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot` |
| Rust | `~/.cargo/bin` (not on PATH by default) |

## Commands

```bash
export PATH="$HOME/.cargo/bin:$PATH"

# Unit tests only (no game install needed)
cargo test -p l2-formats

# Including corpus validation against a real install
LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-formats -- --nocapture

# Differential harness: our Node decoder vs our Rust decoder
powershell -File tools/pl8diff.ps1
```

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
