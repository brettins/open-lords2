<#
  chain.ps1 - record a chain of the original's End Turns, one save per turn.

  HOW TO RECORD A CHAIN (rule 2: never point this at F:\games - it writes saves)

    1. Copy the install once:
         robocopy "F:\games\Lords of the Realm II" E:\games\l2-oracle /E
    2. Start the watcher, which launches the game from the copy:
         powershell -File tools\oracle\chain.ps1 -Launch
       (or start E:\games\l2-oracle\Lords2.exe yourself and run it without -Launch)
    3. Start or load a game. Give your orders, press End Turn; repeat N times.
       Each End Turn prints one "chain-NNN.sav" line.
    4. Quit the game. The watcher exits when Lords2 does (or press Ctrl+C).
    5. Measure it:
         $env:LORDS2_CHAIN='E:\dev\lords2-fixtures\chain'
         cargo test -p l2-game --test differential chain -- --nocapture

  WHAT IT WRITES, in -Out (default E:\dev\lords2-fixtures\chain, outside the repo)

    chain-NNN.sav      a copy of lastturn.sav each time the game rewrites it.
                       Save_RotateAndWrite runs inside Turn_Tick phase 7, so this is
                       the state after one End Turn.
    chain-NNN.json     g_randStateA 0x0058FE28, g_randStateB 0x0058FE08, g_turnCount
                       0x00553240, g_season 0x0057C934, g_year 0x00553EDC read live
                       when the new file was seen. [I] the LFSRs are in no saved block
                       and may have stepped between the write and the read.
    chain-NNN.pre.sav  the save-table blocks (0x004DE960, walked by Save_Write
                       0x004ADE93) read from memory every -PollMs while g_turnPhase
                       0x00569584 is 4 (the players' turn), the last one taken before
                       it left 4, plus Castles.dat: the state before the End Turn
                       that produced chain-NNN.sav, with the player's orders in it.
    chain-NNN.pre.json the same live globals at that snapshot.
#>
[CmdletBinding()]
param(
  [string]$Dir = 'E:\games\l2-oracle',
  [string]$Out = 'E:\dev\lords2-fixtures\chain',
  [int]$PollMs = 200,
  [switch]$Launch,
  # Hand the game the foreground every poll: it quits when not active (runtime.ps1).
  [switch]$KeepFocus,
  # Stop after this many saves (0 = until the game exits).
  [int]$Count = 0
)
$ErrorActionPreference = 'Stop'
if ($Dir -like 'F:\games*') { throw "rule 2: $Dir is a read-only install; copy it (see the header)" }
$exe = Join-Path $Dir 'Lords2.exe'
if (-not (Test-Path $exe)) { throw "no Lords2.exe in $Dir" }
New-Item -ItemType Directory -Force $Out | Out-Null

Add-Type @'
using System;
using System.IO;
using System.Runtime.InteropServices;
public static class Chain {
  [DllImport("kernel32.dll", SetLastError=true)]
  static extern IntPtr OpenProcess(uint da, bool inherit, int pid);
  [DllImport("kernel32.dll", SetLastError=true)]
  static extern bool ReadProcessMemory(IntPtr h, IntPtr addr, byte[] buf, int size, out IntPtr read);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();

  public static uint[] Va; public static int[] Len; public static int Total;

  // The block table Save_Write 0x004ADE93 walks: {u32 va, u32 len} at 0x004DE960,
  // ended by len 0, at most 0xE1 entries (l2_formats::save::Layout).
  public static void LoadTable(string exe) {
    byte[] b = File.ReadAllBytes(exe);
    int pe = BitConverter.ToInt32(b, 0x3C), n = BitConverter.ToUInt16(b, pe + 6);
    int sec = pe + 24 + BitConverter.ToUInt16(b, pe + 20);
    Func<uint, uint> u32 = x => {
      uint rva = x - 0x400000;
      for (int i = 0; i < n; i++) {
        int o = sec + i * 40;
        uint vaddr = BitConverter.ToUInt32(b, o + 12), raw = BitConverter.ToUInt32(b, o + 16);
        uint ptr = BitConverter.ToUInt32(b, o + 20);
        if (rva >= vaddr && rva - vaddr + 4 <= raw) return BitConverter.ToUInt32(b, (int)(ptr + rva - vaddr));
      }
      throw new Exception("block table outside the file");
    };
    var va = new System.Collections.Generic.List<uint>(); var len = new System.Collections.Generic.List<int>();
    Total = 0;
    for (uint i = 0; i < 0xE1; i++) {
      uint a = u32(0x004DE960 + i * 8), l = u32(0x004DE964 + i * 8);
      if (l == 0) break;
      va.Add(a); len.Add((int)l); Total += (int)l;
    }
    Va = va.ToArray(); Len = len.ToArray();
  }

  // Null when the process is gone or any block fails to read.
  public static byte[] Snap(int pid) {
    IntPtr h = OpenProcess(0x0410, false, pid);
    if (h == IntPtr.Zero) return null;
    try {
      byte[] all = new byte[Total]; int at = 0; IntPtr got;
      for (int i = 0; i < Va.Length; i++) {
        byte[] buf = new byte[Len[i]];
        if (!ReadProcessMemory(h, (IntPtr)(long)Va[i], buf, Len[i], out got)) return null;
        Buffer.BlockCopy(buf, 0, all, at, Len[i]); at += Len[i];
      }
      return all;
    } finally { CloseHandle(h); }
  }

  public static long I32(int pid, uint va) {
    IntPtr h = OpenProcess(0x0410, false, pid);
    if (h == IntPtr.Zero) return long.MinValue;
    try {
      byte[] b = new byte[4]; IntPtr got;
      if (!ReadProcessMemory(h, (IntPtr)(long)va, b, 4, out got)) return long.MinValue;
      return BitConverter.ToInt32(b, 0);
    } finally { CloseHandle(h); }
  }

  // runtime.ps1's RuntimeW32.Focus: the game quits when it is not active.
  public static void Focus(IntPtr w) {
    uint d; uint fg = GetWindowThreadProcessId(GetForegroundWindow(), out d), us = GetCurrentThreadId();
    bool at = fg != 0 && fg != us && AttachThreadInput(us, fg, true);
    try { SetForegroundWindow(w); } finally { if (at) AttachThreadInput(us, fg, false); }
  }
}
'@

[Chain]::LoadTable($exe)
$saveLen = [Chain]::Total + 16 * 0x3200
Write-Host "block table: $([Chain]::Va.Length) blocks, $([Chain]::Total) bytes; a save is $saveLen" -ForegroundColor DarkGray

$G = [ordered]@{ randA = 0x0058FE28; randB = 0x0058FE08; turn = 0x00553240
                 season = 0x0057C934; year = 0x00553EDC; phase = 0x00569584 }
function Read-Globals([int]$procId) {
  $o = [ordered]@{}
  foreach ($k in $G.Keys) { $o[$k] = [Chain]::I32($procId, [uint32]$G[$k]) }
  $o['randA'] = $o['randA'] -band 0xFFFFFFFFL; $o['randB'] = $o['randB'] -band 0xFFFFFFFFL
  $o
}
function Write-Json($path, $obj) {
  $obj['taken'] = (Get-Date).ToString('o')
  ($obj | ConvertTo-Json) | Out-File -Encoding utf8 $path
}

$n = 1
Get-ChildItem $Out -Filter 'chain-*.sav' -ErrorAction SilentlyContinue | ForEach-Object {
  if ($_.Name -match '^chain-(\d+)\.sav$') { $n = [Math]::Max($n, [int]$Matches[1] + 1) }
}
$last = Join-Path $Dir 'lastturn.sav'
$seen = if (Test-Path $last) { (Get-Item $last).LastWriteTimeUtc } else { [datetime]::MinValue }

$launched = $null
if ($Launch) {
  $launched = Start-Process -FilePath $exe -WorkingDirectory $Dir -PassThru
  for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep -Milliseconds 250
    $launched.Refresh()
    if ($launched.MainWindowHandle -ne [IntPtr]::Zero) { [Chain]::Focus($launched.MainWindowHandle); break }
  }
}

$pre = $null; $preG = $null; $prevPhase = -1; $procId = 0; $written = 0
Write-Host "watching $last -> $Out (next is chain-$('{0:D3}' -f $n))" -ForegroundColor Yellow
while ($true) {
  $p = Get-Process -Name 'Lords2' -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1
  if (-not $p) {
    if ($procId) { Write-Host "Lords2 exited" -ForegroundColor Yellow; break }
    Start-Sleep -Milliseconds 500; continue
  }
  $procId = $p.Id
  if ($KeepFocus) { [Chain]::Focus($p.MainWindowHandle) }

  $phase = [Chain]::I32($procId, [uint32]$G.phase)
  if ($phase -eq 4) {
    $snap = [Chain]::Snap($procId)
    if ($snap) { $pre = $snap; $preG = Read-Globals $procId }
  } elseif ($prevPhase -eq 4 -and $pre) {
    $name = 'chain-{0:D3}.pre' -f $n
    $castles = [IO.File]::ReadAllBytes((Join-Path $Dir 'Castles.dat'))
    $fs = [IO.File]::Create((Join-Path $Out "$name.sav"))
    try { $fs.Write($pre, 0, $pre.Length); $fs.Write($castles, 0, $castles.Length) } finally { $fs.Close() }
    Write-Json (Join-Path $Out "$name.json") $preG
    Write-Host "$name.sav  turn $($preG.turn) season $($preG.season) (phase 4 -> $phase)"
    $pre = $null
  }
  if ($phase -ne [long]::MinValue) { $prevPhase = $phase }

  if (Test-Path $last) {
    $it = Get-Item $last
    if ($it.LastWriteTimeUtc -ne $seen -and $it.Length -eq $saveLen) {
      $name = 'chain-{0:D3}' -f $n
      try {
        Start-Sleep -Milliseconds 150
        Copy-Item $last (Join-Path $Out "$name.sav") -Force
        $seen = (Get-Item $last).LastWriteTimeUtc
        $g = Read-Globals $procId
        Write-Json (Join-Path $Out "$name.json") $g
        Write-Host "$name.sav      turn $($g.turn) season $($g.season) year $($g.year) randA $($g.randA) randB $($g.randB)" -ForegroundColor Green
        $n++; $written++
        if ($Count -and $written -ge $Count) { break }
      } catch { Write-Host "retrying $last ($($_.Exception.Message))" -ForegroundColor DarkGray }
    }
  }
  Start-Sleep -Milliseconds $PollMs
}
