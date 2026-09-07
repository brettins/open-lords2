<#
  Builds the proxy dplayx.dll. Must be 32-bit: Lords2.exe is PE32.

  Mirrors native/ddraw-proxy/build.ps1. The extra check here is the ordinal
  table: the game imports DPLAYX by ordinal, so an export at the wrong ordinal
  fails to load the process at all, with no log to explain it.
#>
[CmdletBinding()]
param(
  [string]$VcVars = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
)

$dir = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not (Test-Path $VcVars)) { Write-Host "vcvarsall not found: $VcVars"; exit 1 }

# /W4 but not /WX: naked functions and inline asm draw warnings that are the point.
$cmd = "call `"$VcVars`" x86 >nul 2>&1 && cd /d `"$dir`" && " +
       "cl /nologo /LD /MT /W3 /O2 dplay_proxy.cpp /link /DEF:dplayx.def /OUT:dplayx.dll"
cmd /c $cmd
if ($LASTEXITCODE -ne 0) { Write-Host "build failed"; exit 1 }

$dll = Join-Path $dir "dplayx.dll"
if (-not (Test-Path $dll)) { Write-Host "no dplayx.dll produced"; exit 1 }

$bytes = [System.IO.File]::ReadAllBytes($dll)
$pe = [BitConverter]::ToInt32($bytes, 0x3c)
$machine = [BitConverter]::ToUInt16($bytes, $pe + 4)
$arch = if ($machine -eq 0x14c) { "x86 (PE32)" } else { "0x{0:x} - WRONG" -f $machine }
Write-Host "built $dll"
Write-Host "  machine: $arch"

# Walk our own export directory and confirm ordinal 1 and 2 are the two the
# game imports. Ordinal, not name, is what the loader matches here.
$optSize = [BitConverter]::ToUInt16($bytes, $pe + 20)
$opt = $pe + 24
$nsec = [BitConverter]::ToUInt16($bytes, $pe + 6)
$secOff = $opt + $optSize
$secs = @()
for ($i = 0; $i -lt $nsec; $i++) {
  $o = $secOff + $i * 40
  $secs += [pscustomobject]@{
    va  = [BitConverter]::ToUInt32($bytes, $o + 12)
    vs  = [BitConverter]::ToUInt32($bytes, $o + 8)
    raw = [BitConverter]::ToUInt32($bytes, $o + 20)
    rs  = [BitConverter]::ToUInt32($bytes, $o + 16)
  }
}
function R2O($r) {
  foreach ($s in $secs) {
    $size = [Math]::Max($s.vs, $s.rs)
    if ($r -ge $s.va -and $r -lt $s.va + $size) { return $s.raw + ($r - $s.va) }
  }
  return -1
}
function CStr($o) {
  $e = $o
  while ($bytes[$e] -ne 0) { $e++ }
  return [System.Text.Encoding]::ASCII.GetString($bytes, $o, $e - $o)
}
$expRva = [BitConverter]::ToUInt32($bytes, $opt + 96)
$eo = R2O $expRva
$ordBase = [BitConverter]::ToUInt32($bytes, $eo + 16)
$nname   = [BitConverter]::ToUInt32($bytes, $eo + 24)
$names   = R2O ([BitConverter]::ToUInt32($bytes, $eo + 32))
$ords    = R2O ([BitConverter]::ToUInt32($bytes, $eo + 36))
$seen = @{}
for ($i = 0; $i -lt $nname; $i++) {
  $n = CStr (R2O ([BitConverter]::ToUInt32($bytes, $names + $i * 4)))
  $oi = [BitConverter]::ToUInt16($bytes, $ords + $i * 2) + $ordBase
  $seen[[int]$oi] = $n
  Write-Host ("  export @{0} {1}" -f $oi, $n)
}
$ok = ($seen[1] -eq 'DirectPlayCreate') -and ($seen[2] -eq 'DirectPlayEnumerateA') -and ($seen[11] -eq 'gdwDPlaySPRefCount')
Write-Host "  ordinals 1=DirectPlayCreate 2=DirectPlayEnumerateA 11=gdwDPlaySPRefCount: $ok"
if (-not $ok) { exit 1 }
