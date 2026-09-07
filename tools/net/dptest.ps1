<#
  Builds and runs tools/net/dptest.cpp, a 32-bit console exerciser for
  DirectPlay. Run it against the real dplayx.dll for ground truth, and against
  our proxy to prove the proxy's thunks are harmless before risking a fullscreen
  game session.

    powershell -File tools/net/dptest.ps1                 # real dplayx
    powershell -File tools/net/dptest.ps1 -Proxy          # through the proxy
#>
[CmdletBinding()]
param(
  [string]$VcVars = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat",
  [switch]$Proxy,
  [string]$Dll = ""
)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Split-Path -Parent (Split-Path -Parent $here)
$out  = Join-Path $env:TEMP "l2dptest"
if (-not (Test-Path $out)) { New-Item -ItemType Directory -Force $out | Out-Null }

if (-not $Dll) {
  $Dll = if ($Proxy) { Join-Path $repo "native\dplay-proxy\dplayx.dll" } else { "C:\Windows\SysWOW64\dplayx.dll" }
}
if (-not (Test-Path $Dll)) { Write-Host "dll not found: $Dll"; exit 1 }

$cmd = "call `"$VcVars`" x86 >nul 2>&1 && cd /d `"$out`" && " +
       "cl /nologo /MT /W3 /O2 `"$here\dptest.cpp`" /Fe:dptest.exe >nul"
cmd /c $cmd
if ($LASTEXITCODE -ne 0) { Write-Host "build failed"; exit 1 }

# The proxy writes its log beside itself, so it lands next to the DLL under test.
$log = Join-Path (Split-Path -Parent $Dll) "l2dplay.log"
if ($Proxy -and (Test-Path $log)) { Remove-Item $log -Force }

& (Join-Path $out "dptest.exe") $Dll

if ($Proxy -and (Test-Path $log)) {
  Write-Host "`n== proxy log ($log) =="
  Get-Content $log
}
