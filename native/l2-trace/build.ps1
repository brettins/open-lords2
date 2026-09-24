<#
  Builds the headless-trace ddraw.dll and dsound.dll (x86; Lords2.exe is PE32) into -Out.
  Built DLLs are never committed. Then:
    tools\oracle\trace-install.ps1          # E:\games\l2-trace from E:\games\l2-oracle
    tools\oracle\trace.ps1 -Demo            # launch hidden, title menu, one click; JSON line
  Switches the DLL reads from the environment (trace.ps1 sets the first three):
    L2TRACE_GATE=1        lockstep: block at each present until the controller raises `gate`
    L2TRACE_STEP=n        virtual ms per timeGetTime/GetTickCount read (default 1)
    L2TRACE_DUMP=dir      8-bit BMP of every present whose hash changed
    L2TRACE_FILMS=skip    Smk_Play takes its failed-open branch instead of playing
    L2TRACE_TIMERS=drop   drop timeSetEvent instead of running it at presents
    L2TRACE_PRESENT_MS=n  extra virtual ms per present (default 0)
    L2TRACE_REALTIME=1    leave the clock imports alone
    L2TRACE_EVENTS=1      detours (hooktable.h, from gen-hooks.js) write the event ring (events.h);
                          toolsoracle	race-run.ps1 drains it to JSONL
  Output beside the DLLs: l2trace.log (patches, GAP lines, FRAME hash per change), l2trace-dsound.log.
#>
[CmdletBinding()]
param(
  [string]$Out = "C:\Temp\l2-trace-build",
  [string]$VcVars = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
)
$ErrorActionPreference = 'Stop'
$dir = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not (Test-Path $VcVars)) { throw "vcvarsall not found: $VcVars" }
New-Item -ItemType Directory -Force $Out | Out-Null

$flags = "/nologo /LD /MT /W4 /O2 /EHsc /std:c++17 /Fo`"$Out\\`""
$libs = "user32.lib kernel32.lib gdi32.lib"
$cmd = "call `"$VcVars`" x86 >nul 2>&1 && cd /d `"$dir`" && " +
  "cl $flags ddraw_fake.cpp hooks.cpp frame.cpp common.cpp events.cpp detour.cpp $libs /link /DEF:ddraw.def " +
  "/IMPLIB:`"$Out\ddraw.lib`" /OUT:`"$Out\ddraw.dll`" && " +
  "cl $flags dsound_null.cpp common.cpp $libs /link /DEF:dsound.def " +
  "/IMPLIB:`"$Out\dsound.lib`" /OUT:`"$Out\dsound.dll`""
cmd /c $cmd
if ($LASTEXITCODE -ne 0) { throw "build failed" }
foreach ($n in 'ddraw.dll', 'dsound.dll') {
  $b = [IO.File]::ReadAllBytes((Join-Path $Out $n))
  $m = [BitConverter]::ToUInt16($b, [BitConverter]::ToInt32($b, 0x3c) + 4)
  if ($m -ne 0x14c) { throw "$n is not x86" }
  Write-Host "built $Out\$n"
}
