<#
  kingdom.ps1 - check docs/kingdom.md's economic tables against the binary.

  Companion to tables.ps1, same idea and same justification. C11 established
  that every economic constant lives in Lords2.exe rather than a data file, and
  C10 established that the printed manual is not an oracle either - it has been
  caught wrong twice. So the binary is the only authority, and until now nothing
  had actually gone and read it: docs/kingdom.md's numbers came out of a
  decompiler listing, and crates/l2-kingdom was built from that document.

  This reads the addresses kingdom.md names and compares them against the values
  kingdom.md states. A disagreement means either the document misread the
  binary, or the address is wrong - both worth knowing before more code is
  written on top.

  Expected values below are transcribed from docs/kingdom.md, section noted per
  table. They are deliberately written out rather than parsed from the markdown:
  a parser that silently matched nothing would report success.

  tables.ps1 established that the file and live memory agree for the battle
  tables, so this reads the file by default - no process, no window, no focus
  taken. Pass -Source Live to check that claim here too.
#>
[CmdletBinding()]
param(
  [ValidateSet('File','Live')]
  [string]$Source = 'File',
  [string]$Exe = 'F:\games\Lords of the Realm II\Lords2.exe'
)

$ErrorActionPreference = 'Stop'
$IMAGE_BASE = 0x400000

# name, address, expected values, element width, and where kingdom.md says it.
$CHECKS = @(
  # {threshold, band} pairs, not a bare threshold array - and the "else 4" case
  # kingdom.md describes is an explicit fifth pair (100, 4), not a fallthrough.
  # The layout is confirmed by arithmetic: five pairs of int is 40 bytes, and
  # 0x004D6520 + 40 is exactly 0x004D6548, where g_healthHappiness begins.
  @{ Name = 'g_healthBandLadder'; Addr = 0x004D6520; Width = 4; Ref = 'sec 4.2'
     Expect = @(10,0, 35,1, 65,2, 90,3, 100,4) }
  @{ Name = 'g_healthHappiness'; Addr = 0x004D6548; Width = 4; Ref = 'sec 4.2'
     Expect = @(-10, -5, 0, 1, 2) }
  # Two ints per castle level, not one, with both columns holding the same
  # number. kingdom.md prints one column and is right about the values and wrong
  # about the stride. Arithmetic again: ten ints is 40 bytes, and 0x004D89E8 + 40
  # is exactly 0x004D8A10, where the garrison caps begin.
  @{ Name = 'g_castleWorkforce'; Addr = 0x004D89E8; Width = 4; Ref = 'sec 7.4'
     Expect = @(200,200, 400,400, 800,800, 1500,1500, 2500,2500) }
  @{ Name = 'g_castleWoodStone'; Addr = 0x004D89C0; Width = 4; Ref = 'sec 7.4'
     Expect = @(400,40, 800,80, 200,1000, 400,2000, 800,3000) }
  # Six slots, five used and a trailing zero. The same stride carries the tax
  # bonus and free-archer tables, which is why each sits 24 bytes after the last.
  @{ Name = 'g_castleGarrisonCap'; Addr = 0x004D8A10; Width = 4; Ref = 'sec 7.4'
     Expect = @(150, 200, 200, 400, 600, 0) }
  @{ Name = 'g_castleTaxBonus'; Addr = 0x004D8A28; Width = 4; Ref = 'sec 7.4'
     Expect = @(50, 75, 100, 125, 150) }
  @{ Name = 'g_castleFreeArchers'; Addr = 0x004D8A40; Width = 4; Ref = 'sec 7.4'
     Expect = @(50, 150, 150, 200, 300) }
  # Twenty {population, percent} pairs; kingdom.md prints the first ten pairs.
  @{ Name = 'g_birthRateLadder'; Addr = 0x004D6308; Width = 4; Ref = 'sec 5.1'
     Expect = @(40,100, 80,70, 100,50, 250,30, 500,20, 700,15, 800,14, 900,13, 1000,12, 1100,11) }

  # --- added with the crate work that replaced l2-kingdom's honest stubs ----
  # Every one of these was read out of the binary rather than out of
  # kingdom.md, so a failure here means the crate is wrong rather than that the
  # document is. Widths come from what the instruction stream loads: the event
  # deck is read with a 16-bit load and everything else with a 32-bit one.

  # int[5][4] by AI lord and difficulty, for a realm holding three or more
  # counties. kingdom.md sec 8.2 gave only the first and last rows.
  @{ Name = 'g_aiGoldGrant'; Addr = 0x004DC1E0; Width = 4; Ref = 'sec 8.2'
     Expect = @(0,0,0,0,  0,400,700,1200,  100,500,800,1400,  0,400,700,1200,  250,600,1100,1800) }
  # The same shape, for a realm below three counties. Uniformly smaller.
  @{ Name = 'g_aiGoldGrantSmall'; Addr = 0x004DC230; Width = 4; Ref = 'sec 8.2'
     Expect = @(0,0,0,0,  0,160,250,400,  40,180,300,500,  0,160,250,400,  100,240,400,600) }

  # The event deck. Not a 24-entry table: 256 i16 slots, 230 of them zero,
  # spanning 0x004D6108 .. 0x004D6308 - which is exactly where
  # g_birthRateLadder above begins, so the two checks pin each other. Only the
  # first 32 slots are listed; the shape - at most one id in every eighth slot,
  # always the last of the eight - is what matters and it holds throughout.
  @{ Name = 'g_eventTable[0..31]'; Addr = 0x004D6108; Width = 2; Ref = 'sec 8.1'
     Expect = @(0,0,0,0,0,0,0,0x87,  0,0,0,0,0,0,0,0,
                0,0,0,0,0,0,0,0x8C,  0,0,0,0,0,0,0,0x8D) }

  # The happiness cost of raising an army, indexed by the percentage of the
  # county taken. 102 entries ending exactly where the merchant price table
  # begins; the first 32 are listed. This is the L2.eng group 85 "From army"
  # term, which kingdom.md sec 12 records as having no writer found.
  @{ Name = 'g_armyHappinessCost[0..31]'; Addr = 0x004D8778; Width = 4; Ref = 'sec 12'
     Expect = @(0,1,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,11,13,15,17,19,21,23,25,27,29,31) }

  # The merchant base sell price, kingdom.md sec 10 - included because it is
  # what bounds g_armyHappinessCost above, so a change to either is caught.
  @{ Name = 'g_goodSellPrice'; Addr = 0x004D8910; Width = 4; Ref = 'sec 10'
     Expect = @(0, 2, 12, 0, 1, 0, 1, 2, 1, 13, 16, 10, 24, 23, 44) }

  # Six {wood, iron} pairs, kingdom.md sec 7.4 - the table Industry_Produce
  # debits when the blacksmith runs.
  @{ Name = 'g_weaponCost'; Addr = 0x004D8990; Width = 4; Ref = 'sec 7.4'
     Expect = @(6,10, 4,4, 3,10, 6,3, 13,0, 4,18) }

  # int[6][5] [rationLevel][healthBand], added to the health meter each season.
  # Thirty ints is 120 bytes and 0x004D64A8 + 120 is 0x004D6520, where
  # g_healthBandLadder begins.
  @{ Name = 'g_healthDeltaTable'; Addr = 0x004D64A8; Width = 4; Ref = 'sec 4.2'
     Expect = @(-8,-10,-13,-16,-20,  -4,-6,-9,-12,-15,  -2,-4,-6,-8,-12,
                8,4,2,1,-1,  12,8,4,2,0,  20,12,6,3,1) }

  # Six percentages by weather band. Six slots, and g_rationTable is not the
  # next thing along - but 0x004D6560 + 24 is where the following table starts,
  # which is what fixes the count at six rather than five.
  @{ Name = 'g_herdWeatherPct'; Addr = 0x004D6560; Width = 4; Ref = 'sec 7.1'
     Expect = @(-2, -10, 5, 0, -5, -10) }

  # Six {divisor, multiplier} pairs: the ration requirement is
  # DivCeil(people, divisor) * multiplier.
  @{ Name = 'g_rationTable'; Addr = 0x004D6738; Width = 4; Ref = 'sec 4.2'
     Expect = @(1,0, 4,1, 2,1, 1,1, 1,2, 1,3) }

  # The AI personality records, six ints of each of two. The base is
  # 0x004D8A58, which is exactly 24 bytes past g_castleFreeArchers, and the
  # stride is 3 x 0x50. Field +0x00 is the farming style AI_ManageFields
  # dispatches on and +0x04 selects one of the three AI tax ladders; the rest
  # are not identified and are pinned only so a change is noticed.
  @{ Name = 'g_aiPersonality[lord 1]'; Addr = 0x004D8A58; Width = 4; Ref = 'sec 8.2'
     Expect = @(1, 2, 100, 500, 5, 12) }
  # Lord 4, three 0x50-byte rows on, and the only lord using a different ladder.
  @{ Name = 'g_aiPersonality[lord 4]'; Addr = 0x004D8D28; Width = 4; Ref = 'sec 8.2'
     Expect = @(9, 1, 50, 1500, 20, 4) }
)

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class KingdomW32 {
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern IntPtr OpenProcess(uint da, bool inherit, int pid);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool ReadProcessMemory(IntPtr h, IntPtr addr, byte[] buf, int size, out IntPtr read);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool CloseHandle(IntPtr h);
}
'@

function Get-Image([string]$path) {
  $b = [System.IO.File]::ReadAllBytes($path)
  $peOff = [BitConverter]::ToInt32($b, 0x3C)
  $optOff = $peOff + 24
  $n = [BitConverter]::ToUInt16($b, $peOff + 6)
  $optSize = [BitConverter]::ToUInt16($b, $peOff + 20)
  $secOff = $optOff + $optSize
  $sections = @()
  for ($i = 0; $i -lt $n; $i++) {
    $o = $secOff + $i * 40
    $sections += [pscustomobject]@{
      VSize = [BitConverter]::ToUInt32($b, $o + 8); VAddr = [BitConverter]::ToUInt32($b, $o + 12)
      RawSize = [BitConverter]::ToUInt32($b, $o + 16); RawPtr = [BitConverter]::ToUInt32($b, $o + 20)
    }
  }
  return @{ Bytes = $b; Sections = $sections }
}

function Read-Bytes($image, [uint32]$va, [int]$len) {
  $rva = $va - $IMAGE_BASE
  foreach ($s in $image.Sections) {
    if ($rva -ge $s.VAddr -and $rva -lt ($s.VAddr + [Math]::Max($s.VSize, $s.RawSize))) {
      $off = $s.RawPtr + ($rva - $s.VAddr)
      return ,($image.Bytes[$off..($off + $len - 1)])
    }
  }
  throw "0x$($va.ToString('x8')) is in no section"
}

function Read-Live([int]$targetPid, [uint32]$va, [int]$len) {
  $h = [KingdomW32]::OpenProcess(0x0010 -bor 0x0400, $false, $targetPid)
  if ($h -eq [IntPtr]::Zero) { throw "OpenProcess failed" }
  try {
    $buf = New-Object byte[] $len; $read = [IntPtr]::Zero
    if (-not [KingdomW32]::ReadProcessMemory($h, [IntPtr][int64]$va, $buf, $len, [ref]$read)) {
      throw "ReadProcessMemory failed at 0x$($va.ToString('x8'))"
    }
    return ,$buf
  } finally { [KingdomW32]::CloseHandle($h) | Out-Null }
}

$image = Get-Image $Exe
$targetPid = 0
$launched = $null
if ($Source -eq 'Live') {
  $live = Get-Process -Name 'Lords2' -ErrorAction SilentlyContinue
  if ($live) { $targetPid = $live[0].Id }
  else {
    $launched = Start-Process -FilePath $Exe -WorkingDirectory (Split-Path $Exe) -PassThru
    for ($i = 0; $i -lt 60; $i++) {
      Start-Sleep -Milliseconds 500
      $live = Get-Process -Name 'Lords2' -ErrorAction SilentlyContinue
      if ($live) {
        $targetPid = $live[0].Id
        try { Read-Live $targetPid ([uint32]0x004D6520) 4 | Out-Null; break } catch { }
      }
    }
    if (-not $targetPid) { throw "Lords2 never became readable" }
  }
}

Write-Host "oracle: $Exe   source: $Source" -ForegroundColor Green
$pass = 0; $fail = 0
try {
  foreach ($c in $CHECKS) {
    $len = $c.Expect.Count * $c.Width
    $bytes = if ($Source -eq 'Live') { Read-Live $targetPid ([uint32]$c.Addr) $len }
             else { Read-Bytes $image ([uint32]$c.Addr) $len }
    $got = @()
    for ($i = 0; $i -lt $bytes.Length; $i += $c.Width) {
      $got += if ($c.Width -eq 2) { [int][BitConverter]::ToInt16($bytes, $i) }
              else { [BitConverter]::ToInt32($bytes, $i) }
    }
    $same = -not (Compare-Object $got $c.Expect -SyncWindow 0)
    if ($same) {
      $pass++
      Write-Host ("  PASS  {0,-22} {1}  {2}" -f $c.Name, $c.Ref, ($got -join ', ')) -ForegroundColor Green
    } else {
      $fail++
      Write-Host ("  FAIL  {0,-22} {1}" -f $c.Name, $c.Ref) -ForegroundColor Red
      Write-Host ("        kingdom.md says: " + ($c.Expect -join ', ')) -ForegroundColor DarkGray
      Write-Host ("        binary says:     " + ($got -join ', ')) -ForegroundColor Yellow
    }
  }
} finally {
  if ($launched) {
    Stop-Process -Id $launched.Id -Force -ErrorAction SilentlyContinue
    for ($i = 0; $i -lt 40; $i++) {
      $left = Get-Process -Name 'Lords2' -ErrorAction SilentlyContinue
      if (-not $left) { break }
      $left | Stop-Process -Force -ErrorAction SilentlyContinue
      Start-Sleep -Milliseconds 250
    }
    if (Get-Process -Name 'Lords2' -ErrorAction SilentlyContinue) {
      Write-Warning "a Lords2 process is still running"
    } else { Write-Host "no Lords2 processes remain" -ForegroundColor Green }
  }
}

Write-Host ""
Write-Host "$pass passed, $fail failed" -ForegroundColor $(if ($fail) { 'Red' } else { 'Green' })
if ($fail) { exit 1 }
