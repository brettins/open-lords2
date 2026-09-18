<#
  changed.ps1 - test only the crates this branch touches.

    powershell -File tools/run/changed.ps1              # vs main
    powershell -File tools/run/changed.ps1 -Base HEAD~1
    powershell -File tools/run/changed.ps1 -List        # print the crates, run nothing

  Feature agents run this. The integrator runs the full suite once per merge.
  LORDS2_DIR / LORDS2_FIXTURES / LORDS2_DOS_DIR pass through unchanged; a crate
  whose fixtures are absent skips its own tests (docs/environment.md).
#>
param(
  [string]$Base = 'main',
  [switch]$List,
  [string[]]$CargoArgs = @()
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $root

$files = git diff --name-only $Base
if ($LASTEXITCODE -ne 0) { throw "git diff --name-only $Base failed" }

$crates = @()
foreach ($f in $files) {
  if ($f -match '^crates/([^/]+)/') { $crates += $Matches[1] }
}
$crates = $crates | Sort-Object -Unique

if (-not $crates) {
  Write-Host "no crates touched vs $Base"
  exit 0
}

Write-Host "crates touched vs ${Base}: $($crates -join ', ')"
if ($List) { exit 0 }

foreach ($v in 'LORDS2_DIR','LORDS2_FIXTURES','LORDS2_DOS_DIR') {
  $val = [Environment]::GetEnvironmentVariable($v)
  if ($val) { Write-Host "  $v = $val" } else { Write-Host "  $v unset (defaults; see docs/environment.md)" }
}

$failed = @()
foreach ($c in $crates) {
  Write-Host ""
  Write-Host "cargo test -p $c"
  # -q: one dot per test, names only for failures; the passing list never reaches a context window.
  & cargo test -q -p $c @CargoArgs
  if ($LASTEXITCODE -ne 0) { $failed += $c }
}

Write-Host ""
if ($failed) {
  Write-Host "FAILED: $($failed -join ', ')"
  exit 1
}
Write-Host "ok: $($crates -join ', ')"
