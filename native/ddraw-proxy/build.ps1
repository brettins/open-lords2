<#
  Builds the proxy ddraw.dll. Must be 32-bit: Lords2.exe is PE32.
#>
[CmdletBinding()]
param(
  [string]$VcVars = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
)

$dir = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not (Test-Path $VcVars)) { Write-Host "vcvarsall not found: $VcVars"; exit 1 }

$cmd = "call `"$VcVars`" x86 >nul 2>&1 && cd /d `"$dir`" && " +
       "cl /nologo /LD /MT /W4 /O2 ddraw_proxy.cpp /link /DEF:ddraw.def /OUT:ddraw.dll"
cmd /c $cmd
if ($LASTEXITCODE -ne 0) { Write-Host "build failed"; exit 1 }

$dll = Join-Path $dir "ddraw.dll"
if (-not (Test-Path $dll)) { Write-Host "no ddraw.dll produced"; exit 1 }

# Confirm it is really 32-bit and exports the undecorated name - a decorated
# _DirectDrawCreate@12 would not satisfy the game's import.
$bytes = [System.IO.File]::ReadAllBytes($dll)
$pe = [BitConverter]::ToInt32($bytes, 0x3c)
$machine = [BitConverter]::ToUInt16($bytes, $pe + 4)
$arch = if ($machine -eq 0x14c) { "x86 (PE32)" } else { "0x{0:x} - WRONG" -f $machine }
Write-Host "built $dll"
Write-Host "  machine: $arch"

$text = [System.Text.Encoding]::ASCII.GetString($bytes)
Write-Host "  exports DirectDrawCreate undecorated: $($text.Contains('DirectDrawCreate') -and -not $text.Contains('_DirectDrawCreate@'))"
