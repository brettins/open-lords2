<#
  pl8diff.ps1 - cross-implementation differential test for the PL8 decoder.

  Runs the Node reference decoder (tools/pl8digest.js) and the Rust crate
  decoder (crates/l2-formats/examples/pl8digest.rs) over the same corpus and
  asserts their digest streams are identical, line for line.

  Two independently written decoders agreeing on every frame is evidence that
  neither has an implementation bug. It is *not* evidence that the format is
  understood - a misreading shared by both would agree just as cleanly. See
  docs/formats/pl8.md for what is actually confirmed about the container.

  Nothing is written to disk: the streams are compared in memory, so no derived
  game data can leak into the repository.

    .\tools\pl8diff.ps1
    .\tools\pl8diff.ps1 -Dir 'F:\games\Lords of the Realm II' -MaxReport 50
#>
[CmdletBinding()]
param(
  [string]$Dir       = $(if ($env:LORDS2_DIR) { $env:LORDS2_DIR } else { 'F:\games\Lords of the Realm II' }),
  [int]$MaxReport    = 20
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

if (-not (Test-Path -LiteralPath $Dir -PathType Container)) {
  Write-Error "asset directory not found: $Dir  (pass -Dir or set LORDS2_DIR)"
}

# rustup installs outside the machine PATH on this box; don't make the caller care.
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
  if (Test-Path (Join-Path $cargoBin 'cargo.exe')) { $env:PATH = "$cargoBin;$env:PATH" }
  else { Write-Error 'cargo not found on PATH and not in ~\.cargo\bin' }
}

Write-Host "corpus: $Dir"
Write-Host 'building rust digest...'
& cargo build --quiet --release --manifest-path (Join-Path $repo 'Cargo.toml') -p l2-formats --example pl8digest
if ($LASTEXITCODE -ne 0) { Write-Error "cargo build failed ($LASTEXITCODE)" }

$rustExe = Join-Path $repo 'target\release\examples\pl8digest.exe'
$nodeJs  = Join-Path $repo 'tools\pl8digest.js'

Write-Host 'running node decoder...'
# @() so a hypothetical one-line stream stays an array rather than collapsing to
# a string, which would then index by character below.
$node = @(& node $nodeJs $Dir)
if ($LASTEXITCODE -ne 0) { Write-Error "node digest failed ($LASTEXITCODE)" }

Write-Host 'running rust decoder...'
$rust = @(& $rustExe $Dir)
if ($LASTEXITCODE -ne 0) { Write-Error "rust digest failed ($LASTEXITCODE)" }

# Compare positionally rather than as sets: a line appearing at the wrong index
# means the two sides disagree about file order or frame count, which is itself
# a divergence worth failing on.
$n = [Math]::Max($node.Count, $rust.Count)
$diffs = New-Object System.Collections.Generic.List[string]
for ($i = 0; $i -lt $n; $i++) {
  $a = if ($i -lt $node.Count) { $node[$i] } else { '<missing>' }
  $b = if ($i -lt $rust.Count) { $rust[$i] } else { '<missing>' }
  if ($a -cne $b) { $diffs.Add("line $($i + 1):`n    node: $a`n    rust: $b") }
}

$hdr     = @($node | Where-Object { $_ -match ' hdr ' })
$frames  = @($node | Where-Object { $_ -notmatch ' hdr ' })
$hashed  = @($frames | Where-Object { $_ -notmatch ' err:' })
$errored = $frames.Count - $hashed.Count
$ok      = @($hdr | Where-Object { $_ -match 'verdict=ok$' }).Count

Write-Host ''
Write-Host "compared $($hdr.Count) files, $($frames.Count) frames: $($hashed.Count) decoded and hashed, $errored refused by both"
Write-Host "$ok files satisfy the end-offset invariant; $($hdr.Count - $ok) do not (see KNOWN_FAILING and storage mode 2)"

if ($diffs.Count -eq 0) {
  Write-Host 'PASS: node and rust digests are identical' -ForegroundColor Green
  exit 0
}

Write-Host "FAIL: $($diffs.Count) divergent line(s)" -ForegroundColor Red
$diffs | Select-Object -First $MaxReport | ForEach-Object { Write-Host "  $_" }
if ($diffs.Count -gt $MaxReport) { Write-Host "  ... $($diffs.Count - $MaxReport) more" }
exit 1
