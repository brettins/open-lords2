<#
  package.ps1 - build a release and zip it for players.

    powershell -File tools/run/package.ps1              # -> dist/open-lords2-<build>.zip
    powershell -File tools/run/package.ps1 -NoBuild

  The zip holds exactly the files listed between `files:begin` and `files:end`.
  CLAUDE.md rule 1: no game asset ever goes in it, and
  crates/l2-testkit/tests/package_contents.rs fails if the list names one.
  The .pdb stays in the target dir, beside the zip's build, to symbolise a
  crash log's backtrace.
#>
param([switch]$NoBuild)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $root

$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
if (-not $NoBuild) {
  cargo build --release -p l2-game
  if ($LASTEXITCODE -ne 0) { throw 'release build failed' }
}
$exe = Join-Path $target 'release\l2-game.exe'

# Source path -> name in the zip.
$files = [ordered]@{
  # files:begin
  "$exe" = 'l2-game.exe'
  'LICENSE' = 'LICENSE.txt'
  'NOTICE' = 'NOTICE.txt'
  'tools/run/README-player.txt' = 'README.txt'
  # files:end
}

$build = (git rev-parse --short HEAD).Trim()
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist "open-lords2-$build"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
foreach ($src in $files.Keys) {
  if (-not (Test-Path $src)) { throw "missing: $src" }
  Copy-Item $src (Join-Path $stage $files[$src])
}
$zip = "$stage.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
Remove-Item -Recurse -Force $stage
Write-Host "wrote $zip"
