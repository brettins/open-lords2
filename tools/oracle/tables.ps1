<#
  tables.ps1 - read the battle constant tables out of the oracle.

  Every number in crates/l2-sim came out of a decompiler listing. That is a
  reading of the binary, not the binary, and C3 in docs/decisions.md is a
  standing reminder of how confidently a wrong reading can present itself. This
  reads the tables straight out of Lords2.exe instead.

  Two sources, deliberately:

    -Source File   maps the virtual address through the PE section headers and
                   reads the bytes off disk. No process, no window, no focus
                   stolen from whatever the user is doing.
    -Source Live   reads the same addresses out of a running Lords2.exe.

  The reason to have both is Rules_InitConstants: some battle constants are
  written at startup rather than stored initialised, and for those the file is
  the wrong answer. Running -Source Both establishes, per table, whether the
  file read is trustworthy. Where the two agree the file is enough forever and
  the game never needs launching again for that table.

  Lords2.exe has no ASLR and a fixed image base of 0x400000, so a virtual
  address is a constant and can be hardcoded.
#>
[CmdletBinding()]
param(
  [ValidateSet('File','Live','Both')]
  [string]$Source = 'File',
  [string]$Exe    = 'F:\games\Lords of the Realm II\Lords2.exe',
  [int]$LaunchWaitMs = 6000
)

$ErrorActionPreference = 'Stop'
$IMAGE_BASE = 0x400000

# addr, name, rows, columns, element width in bytes.
#
# Width matters and is easy to get wrong. g_meleeAttackTable is u16, and read as
# u32 it produces plausible-looking large integers (262149, 387389207) rather
# than obvious garbage - exactly the kind of output C3 warns about, where a
# wrong reading presents itself confidently. The giveaway was that 262149 is
# 0x00040005: two small numbers in a trenchcoat.
$TABLES = @(
  @{ Addr = 0x004D96D0; Name = 'g_troopBattleStats'; Rows = 11; Cols = 5; Width = 4
     Headings = @('maxFigures','footprint','rowMax','weaponClass','moveDelay') }
  @{ Addr = 0x004D97B0; Name = 'g_missileStats'; Rows = 4; Cols = 5; Width = 4
     Headings = @('range/8','reload','substeps','damage','bank') }
  @{ Addr = 0x004D98F8; Name = 'g_meleeAttackTable'; Rows = 11; Cols = 4; Width = 2
     Headings = @('band0','band1','band2','band3') }
)

# The binary's row order, confirmed rather than assumed: g_troopBattleStats'
# weaponClass column is 2 at row 1, 1 at row 5 and 3 at row 7, which pins
# crossbow, bow and catapult to those three rows and leaves only this ordering.
# It is also, exactly, the order of the Troop enum in crates/l2-sim.
$TROOPS = @('Peasants','Crossbowmen','Macemen','Swordsmen','Pikemen','Archers',
            'Knights','Catapults','SiegeTowers','BatteringRams','Oil')

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OracleW32 {
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern IntPtr OpenProcess(uint da, bool inherit, int pid);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool ReadProcessMemory(IntPtr h, IntPtr addr, byte[] buf, int size, out IntPtr read);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool CloseHandle(IntPtr h);
}
'@

function Get-Sections([string]$path) {
  $b = [System.IO.File]::ReadAllBytes($path)
  $peOff = [BitConverter]::ToInt32($b, 0x3C)
  if ([BitConverter]::ToUInt32($b, $peOff) -ne 0x00004550) { throw "not a PE: $path" }
  $optOff = $peOff + 24
  $nSections = [BitConverter]::ToUInt16($b, $peOff + 6)
  $optSize = [BitConverter]::ToUInt16($b, $peOff + 20)
  $secOff = $optOff + $optSize
  $sections = @()
  for ($i = 0; $i -lt $nSections; $i++) {
    $o = $secOff + $i * 40
    $sections += [pscustomobject]@{
      Name    = ([System.Text.Encoding]::ASCII.GetString($b, $o, 8)).TrimEnd([char]0)
      VSize   = [BitConverter]::ToUInt32($b, $o + 8)
      VAddr   = [BitConverter]::ToUInt32($b, $o + 12)
      RawSize = [BitConverter]::ToUInt32($b, $o + 16)
      RawPtr  = [BitConverter]::ToUInt32($b, $o + 20)
    }
  }
  return @{ Bytes = $b; Sections = $sections }
}

function Read-FromFile($image, [uint32]$va, [int]$len) {
  $rva = $va - $IMAGE_BASE
  foreach ($s in $image.Sections) {
    if ($rva -ge $s.VAddr -and $rva -lt ($s.VAddr + [Math]::Max($s.VSize, $s.RawSize))) {
      $off = $s.RawPtr + ($rva - $s.VAddr)
      if (($off + $len) -gt $image.Bytes.Length) {
        throw "0x$($va.ToString('x8')) is past the end of the file (section $($s.Name))"
      }
      return ,($image.Bytes[$off..($off + $len - 1)])
    }
  }
  throw "0x$($va.ToString('x8')) is in no section"
}

function Read-FromProcess([int]$targetPid, [uint32]$va, [int]$len) {
  $h = [OracleW32]::OpenProcess(0x0010 -bor 0x0400, $false, $targetPid)
  if ($h -eq [IntPtr]::Zero) { throw "OpenProcess failed on pid $targetPid" }
  try {
    $buf = New-Object byte[] $len
    $read = [IntPtr]::Zero
    if (-not [OracleW32]::ReadProcessMemory($h, [IntPtr][int64]$va, $buf, $len, [ref]$read)) {
      throw "ReadProcessMemory failed at 0x$($va.ToString('x8'))"
    }
    return ,$buf
  } finally { [OracleW32]::CloseHandle($h) | Out-Null }
}

function ConvertTo-Ints([byte[]]$bytes, [int]$width) {
  $out = @()
  for ($i = 0; $i + $width - 1 -lt $bytes.Length; $i += $width) {
    if ($width -eq 2) { $out += [int][BitConverter]::ToUInt16($bytes, $i) }
    else { $out += [BitConverter]::ToInt32($bytes, $i) }
  }
  return $out
}

function Show-Table($t, [int[]]$ints, [string]$label) {
  Write-Host ""
  Write-Host "$($t.Name)  @ 0x$($t.Addr.ToString('x8'))  [$label]" -ForegroundColor Cyan
  $w = 14
  $head = (' ' * $w) + (($t.Headings | ForEach-Object { '{0,10}' -f $_ }) -join '')
  Write-Host $head -ForegroundColor DarkGray
  for ($r = 0; $r -lt $t.Rows; $r++) {
    $name = if ($t.Rows -eq 11 -and $r -lt $TROOPS.Count) { $TROOPS[$r] } else { "row $r" }
    $cells = @()
    for ($c = 0; $c -lt $t.Cols; $c++) { $cells += '{0,10}' -f $ints[$r * $t.Cols + $c] }
    Write-Host (('{0,-' + $w + '}') -f $name) -NoNewline
    Write-Host ($cells -join '')
  }
}

$image = Get-Sections $Exe
Write-Host "oracle: $Exe" -ForegroundColor Green
Write-Host ("sections: " + (($image.Sections | ForEach-Object { $_.Name }) -join ' '))

$launched = $null
$targetPid = 0
if ($Source -in @('Live','Both')) {
  $existing = Get-Process -Name 'Lords2' -ErrorAction SilentlyContinue
  if ($existing) {
    $targetPid = $existing[0].Id
    Write-Host "attaching to running Lords2 (pid $targetPid)" -ForegroundColor Yellow
  } else {
    Write-Host "launching Lords2 for a live read..." -ForegroundColor Yellow
    $launched = Start-Process -FilePath $Exe -WorkingDirectory (Split-Path $Exe) -PassThru
    $targetPid = $launched.Id
  }

  # Wait for the image to actually be mapped rather than trusting a sleep. The
  # first attempt at this used a flat delay and failed: a PID exists long before
  # .data is readable, and "launched" is not "running".
  $deadline = (Get-Date).AddMilliseconds([Math]::Max($LaunchWaitMs, 20000))
  $ready = $false
  while ((Get-Date) -lt $deadline) {
    # The process we started may not be the one that ends up holding the game,
    # so re-resolve by name each time rather than trusting the launch PID.
    $live = Get-Process -Name 'Lords2' -ErrorAction SilentlyContinue
    if ($live) {
      $targetPid = $live[0].Id
      try {
        $probe = Read-FromProcess $targetPid ([uint32]0x004D96D0) 4
        if ($probe) { $ready = $true; break }
      } catch { }
    }
    Start-Sleep -Milliseconds 500
  }
  if (-not $ready) { throw "Lords2 never became readable (last pid $targetPid)" }
  $base = (Get-Process -Id $targetPid).MainModule.BaseAddress
  Write-Host ("live pid $targetPid, image base 0x{0:x8}" -f [int64]$base) -ForegroundColor Yellow
  if ([int64]$base -ne $IMAGE_BASE) {
    throw "image base moved to 0x$(([int64]$base).ToString('x8')) - every hardcoded address in this project assumes 0x400000"
  }
}

try {
  $mismatch = 0
  foreach ($t in $TABLES) {
    $len = $t.Rows * $t.Cols * $t.Width
    $fileInts = $null; $liveInts = $null
    if ($Source -in @('File','Both')) {
      $fileInts = ConvertTo-Ints (Read-FromFile $image ([uint32]$t.Addr) $len) $t.Width
    }
    if ($Source -in @('Live','Both')) {
      $liveInts = ConvertTo-Ints (Read-FromProcess $targetPid ([uint32]$t.Addr) $len) $t.Width
    }

    if ($Source -eq 'Both') {
      $same = -not (Compare-Object $fileInts $liveInts -SyncWindow 0)
      Show-Table $t $fileInts 'file'
      if ($same) {
        Write-Host "  file and live agree - the file read is authoritative for this table" -ForegroundColor Green
      } else {
        $mismatch++
        Write-Host "  FILE AND LIVE DIFFER - something writes this at startup" -ForegroundColor Red
        Show-Table $t $liveInts 'live'
      }
    } elseif ($Source -eq 'File') {
      Show-Table $t $fileInts 'file'
    } else {
      Show-Table $t $liveInts 'live'
    }
  }
  Write-Host ""
  if ($Source -eq 'Both') {
    if ($mismatch -eq 0) {
      Write-Host "all tables identical on disk and in memory" -ForegroundColor Green
    } else {
      Write-Host "$mismatch table(s) rewritten at startup - use the live values" -ForegroundColor Red
    }
  }
} finally {
  # Agents and tools must not leave the game running.
  # Leave nothing running, and *verify* it rather than assuming. Stop-Process
  # returns before the process is gone, so the first version of this reported
  # success while a Lords2 was still winding down.
  if ($launched) {
    Write-Host "closing the Lords2 we launched" -ForegroundColor Yellow
    Stop-Process -Id $launched.Id -Force -ErrorAction SilentlyContinue
    $gone = $false
    for ($i = 0; $i -lt 40; $i++) {
      $left = Get-Process -Name 'Lords2' -ErrorAction SilentlyContinue
      if (-not $left) { $gone = $true; break }
      $left | Stop-Process -Force -ErrorAction SilentlyContinue
      Start-Sleep -Milliseconds 250
    }
    if ($gone) {
      Write-Host "no Lords2 processes remain" -ForegroundColor Green
    } else {
      Write-Warning "a Lords2 process is still running - kill it before trusting anything else"
    }
  }
}
