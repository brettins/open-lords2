<#
  play.ps1 - build the game, then run a COPY of it, so the two never collide.

  Windows refuses to let a link step overwrite a running executable, so with the
  game open every build in the workspace goes red - not only `cargo build`, but
  `cargo test --workspace`, which links the same binary. It has cost this project
  real time twice: "game is closed. You cant rebuild with it open?"

  `crates/l2-game/build.rs` can move a locked binary aside (Windows allows
  renaming a running exe even though it refuses to overwrite one), but making
  that happen before EVERY link costs a full recompile of `l2-game` on every
  build - measured at 5-7s against 0.19s for a no-op build - which is a
  permanent tax on everyone to cover the minutes a day somebody is playing.

  This costs nothing instead. The copy is what gets locked, so
  `target/<profile>/l2-game.exe` is never held open and no build ever contends
  with a running game. Launch as many instances as you like; each takes its own
  copy.

  WHAT THIS DOES NOT CHANGE: `target/<profile>/l2-game.exe` is still built, at
  the same path, by the same command. An existing desktop shortcut pointing
  straight at it keeps working exactly as before - this script is an addition,
  not a replacement. Point a shortcut at `tools\run\play.cmd` only if you want
  the build-and-never-collide behaviour.

  Usage:
    tools\run\play.ps1               # debug build, then play
    tools\run\play.ps1 -Release      # release build, then play
    tools\run\play.ps1 -NoBuild      # skip the build, play what is there
#>
[CmdletBinding()]
param(
    [switch]$Release,
    [switch]$NoBuild
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
Get-ChildItem -Path $outDir -Filter 'l2-game-live*.exe' -ErrorAction SilentlyContinue | ForEach-Object {
    try { Remove-Item $_.FullName -Force -ErrorAction Stop } catch { }
}

# Take the first copy name that is free, so a second instance does not have to
# wait for the first to quit.
$copy = $null
foreach ($n in 0..19) {
    $candidate = Join-Path $outDir $(if ($n -eq 0) { 'l2-game-live.exe' } else { "l2-game-live-$n.exe" })
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
& $copy @args
exit $LASTEXITCODE
