<#
  play.ps1 - build the game, then run a COPY of it, so the two never collide.

  Windows refuses to let a build replace a running executable, so with the game
  open every build in the workspace goes red - not only `cargo build`, but
  `cargo test --workspace`, which builds the same binary. It is cargo's last
  step that fails, not the linker: "failed to remove file ...\l2-game.exe" /
  "Access is denied. (os error 5)". It has cost this project real time twice:
  "game is closed. You cant rebuild with it open?"

  THIS IS THE PRIMARY MECHANISM, and it makes two promises:

  1. A build succeeds while the game is open. The copy is what gets locked, so
     `target/<profile>/l2-game.exe` is never held open by a game started here.
  2. It always runs the newest build. It builds first, and a failed build
     launches NOTHING - it does not fall back to the last good binary, because
     a launcher that silently ran a stale copy would be worse than the problem.
     `-NoBuild` is the only way to opt out of that, and you asked for it.

  `crates/l2-game/build.rs` also moves a locked binary aside (Windows allows
  renaming a running exe even though it refuses to overwrite one), but only on
  a build that follows a commit, checkout or `git add`. Making it happen before
  EVERY link (`L2_ALWAYS_UNLOCK=1`) recompiles `l2-game` on every build -
  measured at 2.6-3.2s against 0.17-0.21s for a no-op - a permanent tax on
  everyone to cover the minutes a day somebody is playing. This costs one
  19 MB file copy per launch instead. Up to twenty instances each take their
  own copy.

  WHAT THIS DOES NOT CHANGE: `target/<profile>/l2-game.exe` is still built, at
  the same path, by the same command. An existing desktop shortcut pointing
  straight at it keeps working exactly as before - but a game started that way
  locks the build output again, so builds collide with it. Point a shortcut at
  `tools\run\play.cmd` for the two promises above. `docs/environment.md`.

  Usage (everything the launcher does not recognise is passed to the game):
    tools\run\play.ps1 <game dir>             # debug build, then play
    tools\run\play.ps1 -Release <game dir>    # release build, then play
    tools\run\play.ps1 -NoBuild <game dir>    # skip the build, play what is there
    tools\run\play.ps1 <game dir> --no-sound  # any l2-game flag passes through
#>
[CmdletBinding()]
param(
    [switch]$Release,
    [switch]$NoBuild,
    # **Everything else goes to the game.** This parameter used not to exist and
    # the call below splatted `@args` - which an advanced script (the
    # `[CmdletBinding()]` above) does not have. Every argument was a binding
    # error before the build even started, measured: *"A positional parameter
    # cannot be found that accepts argument 'F:\games\Lords of the Realm II'"*.
    # `l2-game` requires a game directory, so the launcher could never have
    # launched the game. `docs/decisions.md` CNEW-launcher-never-launched.
    #
    # One trap left, and it is PowerShell's: a single-dash game flag that is a
    # prefix of a switch here is taken by the switch (`-n` becomes `-NoBuild`).
    # `l2-game`'s flags are `--mods`, `--no-sound`, `-h` and `--help`, and none
    # of them is caught - measured, each arrives intact.
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$GameArgs
)

$ErrorActionPreference = 'Stop'

# The repo root is two levels up from tools\run.
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$profileName = if ($Release) { 'release' } else { 'debug' }
$outDir = Join-Path $root "target\$profileName"
$exe = Join-Path $outDir 'l2-game.exe'

if (-not $NoBuild) {
    Write-Host "building ($profileName)..." -ForegroundColor DarkGray
    Push-Location $root
    try {
        if ($Release) { cargo build --release -p l2-game } else { cargo build -p l2-game }
        if ($LASTEXITCODE -ne 0) {
            Write-Host 'build failed - not launching.' -ForegroundColor Red
            exit $LASTEXITCODE
        }
    } finally {
        Pop-Location
    }
}

if (-not (Test-Path $exe)) {
    Write-Host "no binary at $exe - build first, or drop -NoBuild." -ForegroundColor Red
    exit 1
}

# Sweep copies from previous sessions. A copy still being played refuses to be
# deleted, which is the answer we want rather than an error: it is collected the
# next time nobody is holding it.
#
# The copy names below and this filter come from one stem on purpose. If they
# drift apart the sweep matches nothing - which is silent, because a pattern
# that fails to match raises nothing - and a 19 MB copy is stranded per launch.
$live = 'l2-game-live'
Get-ChildItem -Path $outDir -Filter "$live*.exe" -ErrorAction SilentlyContinue | ForEach-Object {
    try { Remove-Item $_.FullName -Force -ErrorAction Stop } catch { }
}

# Take the first copy name that is free, so a second instance does not have to
# wait for the first to quit.
$copy = $null
foreach ($n in 0..19) {
    $candidate = Join-Path $outDir $(if ($n -eq 0) { "$live.exe" } else { "$live-$n.exe" })
    try {
        Copy-Item $exe $candidate -Force -ErrorAction Stop
        $copy = $candidate
        break
    } catch { }
}

if (-not $copy) {
    # Twenty locked copies means twenty instances, or something else holding
    # them. Running the original still works; it just locks the build output,
    # which is the situation this script exists to avoid - so say so.
    Write-Host 'could not make a free copy; running the build output directly.' -ForegroundColor Yellow
    Write-Host 'builds will fail while this is open.' -ForegroundColor Yellow
    $copy = $exe
}

Write-Host "playing $(Split-Path -Leaf $copy)" -ForegroundColor DarkGray
& $copy @GameArgs
exit $LASTEXITCODE
