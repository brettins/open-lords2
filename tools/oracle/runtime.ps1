<#
  runtime.ps1 - verify the constants Rules_InitConstants writes at startup.

  tables.ps1 and kingdom.ps1 read tables that are stored initialised, and both
  found the file and live memory identical. These are the opposite case, and the
  reason the file/live distinction was built in the first place.

  Rules_InitConstants (0x004983B7) writes its values into uninitialised .data
  with a run of `MOV dword ptr [...], imm`. Before it runs those addresses are
  zero, so reading the executable off disk gives the wrong answer with complete
  confidence - the one situation where the cheap check is actively misleading.
  This launches the game, waits for the writes, and reads the real values.

  THE GAME MUST HOLD THE FOREGROUND, AND THIS SCRIPT GIVES IT.

  The first version of this launched the game and read zeros from every address.
  That was not a fact about the constants: started from a script Lords2 survives
  about 2.6 seconds and its own status.txt ends

      OK :Window moved.
      OK :Not active.

  The launching process keeps the foreground, the game decides it is not active,
  and it exits before Rules_InitConstants has run.

  It is fixable, which corrects D8. `SetForegroundWindow` really does fail from
  a background process - Windows only lets whoever already owns the foreground
  give it away - but the documented workaround applies: AttachThreadInput joins
  our input queue to the current foreground thread's, we count as that owner for
  as long as we stay attached, hand the foreground to the game, and detach. See
  RuntimeW32.Focus below. The foreground is handed back on every sampling pass,
  because anything at all can take it away mid-read.

  So this launches by default. Do not click away while it runs, and do not let
  the machine lock - the secure desktop deactivates the game and it will quit.
  Nothing is corrupted if that happens; the read simply fails and can be redone.
  Pass -NoLaunch to attach to a game that is already running instead.

  (The "Could not read sierra.ini" line in status.txt is a red herring. That
  file ships with no GOG build and is part of the same dead Sierra online stack
  as SNWValid.dll - see docs/netcode.md. The game logs it and carries on.)

  Expected values are transcribed from docs/symbols.json and docs/battle-ai.md.
  Two of them carry weight beyond arithmetic:

    g_grainMaxSacksPerField = 10 is correction C10's evidence. The printed
    manual says up to 5 sacks per field. Long-standing player measurement said
    the manual was wrong. If this reads 10, the players were right and a
    published manual was not.

    g_aiAggressionThreshold = 5 and g_aiSortieThreshold = 260 are the whole
    battle AI. Every field handler attacks above the first; every siege
    defender sorties above the second.

  The game is launched, read, and closed. Nothing is left running.
#>
[CmdletBinding()]
param(
  [string]$Exe = 'F:\games\Lords of the Realm II\Lords2.exe',
  # Launch and hand the game the foreground. Pass -NoLaunch to only attach to a
  # game that is already running.
  [switch]$NoLaunch
)

$ErrorActionPreference = 'Stop'
$IMAGE_BASE = 0x400000

$CONSTS = @(
  @{ Name = 'g_aiAggressionThreshold'; Addr = 0x0057C8B4; Expect = 5
     Note = 'field AI attacks above this % strength advantage' }
  @{ Name = 'g_aiSortieThreshold'; Addr = 0x00552FF0; Expect = 260
     Note = 'siege defenders sortie above this' }
  @{ Name = 'g_moatFillSteps'; Addr = 0x00554090; Expect = 15
     Note = 'terrain raises this many times before a moat cell becomes ground' }
  @{ Name = 'g_grainYieldPerSack'; Addr = 0x0057C8E0; Expect = 12; Note = '' }
  @{ Name = 'g_grainMaxSacksPerField'; Addr = 0x00552FFC; Expect = 10
     Note = 'the manual says 5 - see correction C10' }
  @{ Name = 'g_foodPerHead'; Addr = 0x00567594; Expect = 10; Note = '' }
  @{ Name = 'g_foodPerSack'; Addr = 0x0057CB30; Expect = 6; Note = '' }
  @{ Name = 'g_dairyPerHead'; Addr = 0x00553F60; Expect = 5; Note = '' }
  @{ Name = 'g_grainLabourDivisor'; Addr = 0x0057D34C; Expect = 2; Note = '' }
  @{ Name = 'g_grainLabourDivisorAdv'; Addr = 0x005533BC; Expect = 5; Note = '' }
)

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class RuntimeW32 {
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern IntPtr OpenProcess(uint da, bool inherit, int pid);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool ReadProcessMemory(IntPtr h, IntPtr addr, byte[] buf, int size, out IntPtr read);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool CloseHandle(IntPtr h);

  // Handing the foreground to the game. D8 recorded that SetForegroundWindow
  // "fails silently from a background process", and that is true on its own -
  // Windows only lets the process that already owns the foreground give it
  // away. The documented way round it is to join our input queue to the
  // current foreground thread's with AttachThreadInput, which makes us count
  // as that owner for the duration, hand the foreground over, and detach.
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint attach, uint attachTo, bool fAttach);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();

  public static bool Focus(IntPtr hWnd) {
    if (hWnd == IntPtr.Zero) return false;
    IntPtr fg = GetForegroundWindow();
    uint dummy;
    uint fgThread = GetWindowThreadProcessId(fg, out dummy);
    uint us = GetCurrentThreadId();
    bool attached = false;
    if (fgThread != 0 && fgThread != us) attached = AttachThreadInput(us, fgThread, true);
    try {
      ShowWindow(hWnd, 5 /* SW_SHOW */);
      BringWindowToTop(hWnd);
      return SetForegroundWindow(hWnd);
    } finally {
      if (attached) AttachThreadInput(us, fgThread, false);
    }
  }
}
'@

function Read-Live([int]$targetPid, [uint32]$va, [int]$len) {
  $h = [RuntimeW32]::OpenProcess(0x0010 -bor 0x0400, $false, $targetPid)
  if ($h -eq [IntPtr]::Zero) { throw "OpenProcess failed on pid $targetPid" }
  try {
    $buf = New-Object byte[] $len; $read = [IntPtr]::Zero
    if (-not [RuntimeW32]::ReadProcessMemory($h, [IntPtr][int64]$va, $buf, $len, [ref]$read)) {
      throw "ReadProcessMemory failed at 0x$($va.ToString('x8'))"
    }
    return ,$buf
  } finally { [RuntimeW32]::CloseHandle($h) | Out-Null }
}

function Read-FileDword([string]$path, [uint32]$va) {
  $b = [System.IO.File]::ReadAllBytes($path)
  $peOff = [BitConverter]::ToInt32($b, 0x3C)
  $n = [BitConverter]::ToUInt16($b, $peOff + 6)
  $optSize = [BitConverter]::ToUInt16($b, $peOff + 20)
  $secOff = $peOff + 24 + $optSize
  $rva = $va - $IMAGE_BASE
  for ($i = 0; $i -lt $n; $i++) {
    $o = $secOff + $i * 40
    $vs = [BitConverter]::ToUInt32($b, $o + 8);  $va2 = [BitConverter]::ToUInt32($b, $o + 12)
    $rs = [BitConverter]::ToUInt32($b, $o + 16); $rp = [BitConverter]::ToUInt32($b, $o + 20)
    if ($rva -ge $va2 -and $rva -lt ($va2 + [Math]::Max($vs, $rs))) {
      # Past the raw data is uninitialised .bss - it has no bytes in the file.
      $off = $rp + ($rva - $va2)
      if (($rva - $va2) -ge $rs) { return $null }
      return [BitConverter]::ToInt32($b, $off)
    }
  }
  return $null
}

$launched = $null
$targetPid = 0
$existing = Get-Process -Name 'Lords2' -ErrorAction SilentlyContinue
if ($existing) {
  $targetPid = $existing[0].Id
  Write-Host "attaching to running Lords2 (pid $targetPid)" -ForegroundColor Yellow
} elseif ($NoLaunch) {
  Write-Host ""
  Write-Host "No Lords2 is running, and -NoLaunch was given." -ForegroundColor Yellow
  Write-Host "Start the game yourself, leave it focused, then run this again:" -ForegroundColor Cyan
  Write-Host "  $Exe" -ForegroundColor Cyan
  exit 2
} else {
  Write-Host "launching Lords2 and handing it the foreground" -ForegroundColor Yellow
  Write-Host "(do not click away, and do not let the machine lock, until this finishes)" -ForegroundColor DarkGray
  $launched = Start-Process -FilePath $Exe -WorkingDirectory (Split-Path $Exe) -PassThru

  # Give the game the foreground as soon as it has a window, and keep giving it
  # back. Without this it sees itself deactivated and exits in about 2.6s,
  # before Rules_InitConstants ever runs.
  for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep -Milliseconds 250
    $p = Get-Process -Id $launched.Id -ErrorAction SilentlyContinue
    if (-not $p) { break }
    $p.Refresh()
    if ($p.MainWindowHandle -ne [IntPtr]::Zero) {
      [void][RuntimeW32]::Focus($p.MainWindowHandle)
      break
    }
  }
  $deadline = (Get-Date).AddSeconds(40)
  while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    $live = Get-Process -Name 'Lords2' -ErrorAction SilentlyContinue
    if ($live) {
      $targetPid = $live[0].Id
      try { Read-Live $targetPid ([uint32]0x0057C8B4) 4 | Out-Null; break } catch { }
    }
  }
  if (-not $targetPid) { throw "Lords2 never became readable" }
}

# Sample repeatedly rather than sleeping and hoping.
#
# Rules_InitConstants runs during startup, not at process creation, so there is
# a window in which the addresses are still zero. But this game does not stay
# up: on this machine it exits a few seconds after launch, and the first version
# of this script slept through its whole lifetime and then found no process to
# read. So: start sampling the moment memory is readable, keep the newest sample
# in which every constant is non-zero, and stop as soon as one is complete.
$sample = $null
$deadline = (Get-Date).AddSeconds(40)
while ((Get-Date) -lt $deadline) {
  $attempt = @{}
  $ok = $true
  foreach ($c in $CONSTS) {
    try { $attempt[$c.Name] = [BitConverter]::ToInt32((Read-Live $targetPid ([uint32]$c.Addr) 4), 0) }
    catch { $ok = $false; break }
  }
  if ($ok) {
    $sample = $attempt
    if (-not ($CONSTS | Where-Object { $attempt[$_.Name] -eq 0 })) { break }
  } elseif ($sample) {
    break   # the process is gone, but we already have a full reading
  }
  # Keep handing the foreground back. The game exits the moment it decides it
  # is not active, and anything at all - a notification, a build finishing, the
  # machine locking - can take it away mid-sample.
  if ($launched) {
    $p = Get-Process -Id $launched.Id -ErrorAction SilentlyContinue
    if ($p) {
      $p.Refresh()
      if ($p.MainWindowHandle -ne [IntPtr]::Zero) {
        [void][RuntimeW32]::Focus($p.MainWindowHandle)
      }
    }
  }
  Start-Sleep -Milliseconds 150
}
if (-not $sample) { throw "never managed a complete reading before Lords2 exited" }

$pass = 0; $fail = 0; $provedZero = 0
try {
  Write-Host ""
  foreach ($c in $CONSTS) {
    $got = $sample[$c.Name]
    $onDisk = Read-FileDword $Exe ([uint32]$c.Addr)
    $diskText = if ($null -eq $onDisk) { 'uninitialised' } else { "$onDisk" }
    if ($null -eq $onDisk -or $onDisk -ne $c.Expect) { $provedZero++ }

    if ($got -eq $c.Expect) {
      $pass++
      $line = "  PASS  {0,-26} = {1,-6} (disk: {2})" -f $c.Name, $got, $diskText
      Write-Host $line -ForegroundColor Green
    } else {
      $fail++
      Write-Host ("  FAIL  {0,-26} expected {1}, live memory has {2} (disk: {3})" -f `
        $c.Name, $c.Expect, $got, $diskText) -ForegroundColor Red
    }
    if ($c.Note) { Write-Host ("        " + $c.Note) -ForegroundColor DarkGray }
  }
} finally {
  if ($launched) {
    Write-Host ""
    Write-Host "closing the Lords2 we launched" -ForegroundColor Yellow
    Stop-Process -Id $launched.Id -Force -ErrorAction SilentlyContinue
    $gone = $false
    for ($i = 0; $i -lt 40; $i++) {
      $left = Get-Process -Name 'Lords2' -ErrorAction SilentlyContinue
      if (-not $left) { $gone = $true; break }
      $left | Stop-Process -Force -ErrorAction SilentlyContinue
      Start-Sleep -Milliseconds 250
    }
    if ($gone) { Write-Host "no Lords2 processes remain" -ForegroundColor Green }
    else { Write-Warning "a Lords2 process is still running" }
  }
}

Write-Host ""
Write-Host "$pass passed, $fail failed" -ForegroundColor $(if ($fail) { 'Red' } else { 'Green' })
Write-Host "$provedZero of $($CONSTS.Count) differ from the file - reading the executable alone would have been wrong about them" -ForegroundColor Cyan
if ($fail) { exit 1 }
