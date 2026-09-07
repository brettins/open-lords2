<#
  Builds an instrumented copy of the game beside the original, using hard links,
  and drops the proxy dplayx.dll into it.

  Same reasoning as native/ddraw-proxy/install.ps1: CLAUDE.md rule 2 makes the
  installs read-only, but a proxy DLL has to sit next to the executable. Hard
  links give a second directory entry for every game file at no disk cost and
  without writing to the original. Only our own dplayx.dll is a real file.

  Default destination is deliberately NOT lords2-instrumented: that sandbox
  belongs to the ddraw proxy and this one should be able to run without it.
  Use -WithDDraw to also drop the already-built ddraw proxy in (it is only read,
  never rebuilt, by this script).

  Two endpoints are needed for a real session, so -Instance lets you stamp out
  more than one sandbox: each gets its own directory, its own l2dplay.log and
  its own status.txt.
#>
[CmdletBinding()]
param(
  [string]$Source   = "F:\games\Lords of the Realm II",
  [string]$Dest     = "",
  [string]$Instance = "",
  [switch]$WithDDraw,
  [switch]$Clean
)

if (-not $Dest) {
  $Dest = if ($Instance) { "F:\games\lords2-net-$Instance" } else { "F:\games\lords2-net" }
}

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
Write-Host "hard-linked $linked file(s), $skipped already present -> $Dest"

# status.txt and L2.INI are hard links to the real install, so the game would
# write through them into the read-only directory. Break those two links.
foreach ($writable in @("status.txt", "L2.INI")) {
  $p = Join-Path $Dest $writable
  if (Test-Path $p) {
    $content = [System.IO.File]::ReadAllBytes($p)
    Remove-Item $p -Force
    [System.IO.File]::WriteAllBytes($p, $content)
    Write-Host "  un-linked $writable (the game writes to it)"
  }
}

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$dll = Join-Path $here "dplayx.dll"
if (-not (Test-Path $dll)) { Write-Host "dplayx.dll not built - run build.ps1 first"; exit 1 }
Copy-Item $dll (Join-Path $Dest "dplayx.dll") -Force
Write-Host "installed proxy dplayx.dll"

if ($WithDDraw) {
  $dd = Join-Path (Split-Path -Parent $here) "ddraw-proxy\ddraw.dll"
  if (Test-Path $dd) { Copy-Item $dd (Join-Path $Dest "ddraw.dll") -Force; Write-Host "installed ddraw proxy too" }
  else { Write-Host "ddraw proxy not built - skipping" }
}

foreach ($n in @("dplayx.dll", "ddraw.dll")) {
  $orig = Join-Path $Source $n
  if (Test-Path $orig) { Write-Host "WARNING: the original install now contains a $n - it should not" }
}

Remove-Item (Join-Path $Dest "l2dplay.log") -Force -ErrorAction SilentlyContinue
Write-Host "run: `"$Dest\Lords2.exe`"   log: $Dest\l2dplay.log"
