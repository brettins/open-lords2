<#
  (Re)creates the scratch install the headless trace runs in (native/l2-trace).
  Bulk assets are hard links to -Source (same volume, read-only use); every other file is
  COPIED, because the game writes L2.INI, lords2.inf, *.sav, USER.SKR and status.txt and a
  write through a hard link would change -Source too. Refuses any F:\ path (CLAUDE.md rule 2).
#>
[CmdletBinding()]
param(
  [string]$Source = 'E:\games\l2-oracle',
  [string]$Dest = 'E:\games\l2-trace',
  [string]$Dlls = 'C:\Temp\l2-trace-build'
)
$ErrorActionPreference = 'Stop'
foreach ($p in $Source, $Dest) {
  $full = [IO.Path]::GetFullPath($p)
  if ($full -match '^[Ff]:') { throw "refusing $full - the F:\ installs are read-only" }
}
$Source = [IO.Path]::GetFullPath($Source); $Dest = [IO.Path]::GetFullPath($Dest)
if ($Source -eq $Dest) { throw "source and destination are the same" }
if ((Split-Path -Qualifier $Source) -ne (Split-Path -Qualifier $Dest)) { throw "hard links need one volume" }
foreach ($n in 'ddraw.dll', 'dsound.dll') {
  if (-not (Test-Path (Join-Path $Dlls $n))) { throw "$n missing in $Dlls - run native\l2-trace\build.ps1" }
}

# Read-only bulk: art, films, sound, the manual.
$linkExt = '.pl8', '.256', '.smk', '.wav', '.saf', '.pdf', '.zip', '.hlp'

if (Test-Path $Dest) { Remove-Item -Recurse -Force $Dest }
New-Item -ItemType Directory $Dest | Out-Null
$linked = 0; $copied = 0
Get-ChildItem -File $Source | ForEach-Object {
  $to = Join-Path $Dest $_.Name
  if ($linkExt -contains $_.Extension.ToLower()) {
    New-Item -ItemType HardLink -Path $to -Target $_.FullName | Out-Null; $linked++
  } else {
    Copy-Item $_.FullName $to; $copied++
  }
}
Get-ChildItem -Directory $Source | ForEach-Object { Copy-Item -Recurse $_.FullName (Join-Path $Dest $_.Name); $copied++ }
Copy-Item (Join-Path $Dlls 'ddraw.dll'), (Join-Path $Dlls 'dsound.dll') $Dest
Write-Host "trace install $Dest : $linked linked, $copied copied, ddraw.dll + dsound.dll from $Dlls"
