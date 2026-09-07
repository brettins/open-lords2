<#
  Builds an instrumented copy of the game beside the original, using hard links.

  CLAUDE.md rule 2 says the game installs are read-only, but a proxy DLL has to
  sit next to the executable. Hard links resolve that: every game file appears
  in the new directory without being copied, costing no disk space, and the
  original install is never written to. Only our own ddraw.dll is a real file.

  Hard links require the same volume, so the sandbox lives on the game's drive.
#>
[CmdletBinding()]
param(
  [string]$Source = "F:\games\Lords of the Realm II",
  [string]$Dest   = "F:\games\lords2-instrumented",
  [switch]$Clean
)

if (-not (Test-Path $Source)) { Write-Host "source not found: $Source"; exit 1 }

$srcRoot = (Get-Item $Source).PSDrive.Name
$dstRoot = Split-Path -Qualifier $Dest
if ("$srcRoot`:" -ne $dstRoot) {
  Write-Host "hard links need one volume: source is ${srcRoot}: but dest is $dstRoot"
  exit 1
}

if ($Clean -and (Test-Path $Dest)) {
  Remove-Item $Dest -Recurse -Force
  Write-Host "removed $Dest"
}
if (-not (Test-Path $Dest)) { New-Item -ItemType Directory -Force $Dest | Out-Null }

$linked = 0; $skipped = 0
foreach ($f in Get-ChildItem -Path $Source -File) {
  $target = Join-Path $Dest $f.Name
  if (Test-Path $target) { $skipped++; continue }
  try {
    New-Item -ItemType HardLink -Path $target -Target $f.FullName -ErrorAction Stop | Out-Null
    $linked++
  } catch {
    Write-Host "  could not link $($f.Name): $($_.Exception.Message)"
  }
}
foreach ($d in Get-ChildItem -Path $Source -Directory) {
  Write-Host "  note: subdirectory '$($d.Name)' not linked (install is expected to be flat)"
}
Write-Host "hard-linked $linked file(s), $skipped already present -> $Dest"

# Our DLL is a real file, deliberately: it is the only thing that differs.
$dll = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) "ddraw.dll"
if (-not (Test-Path $dll)) { Write-Host "ddraw.dll not built - run build.ps1 first"; exit 1 }
Copy-Item $dll (Join-Path $Dest "ddraw.dll") -Force
Write-Host "installed proxy ddraw.dll"

$orig = Join-Path $Source "ddraw.dll"
if (Test-Path $orig) { Write-Host "WARNING: the original install now contains a ddraw.dll - it should not" }
Write-Host "run: `"$Dest\Lords2.exe`""
