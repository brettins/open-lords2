<#
  battlestate.ps1 - read live battle state out of a running Lords2.exe.

  Lords2.exe has no ASLR (ImageBase 0x400000), so every address below is fixed.
  See docs/battle.md for the field maps; this script only formats what is there.

    powershell -File tools/battle/battlestate.ps1                 # figures + units
    powershell -File tools/battle/battlestate.ps1 -Action units
    powershell -File tools/battle/battlestate.ps1 -Action cell -X 40 -Y 20
    powershell -File tools/battle/battlestate.ps1 -Action raw -Addr 0x554480 -Length 432

  It never writes to the process. Nothing here launches or closes the game.
#>
[CmdletBinding()]
param(
  [ValidateSet('all', 'figures', 'units', 'globals', 'cell', 'raw')]
  [string]$Action = 'all',
  [string]$Name = 'Lords2',
  [int]$ProcId = 0,
  [int]$X = 0,
  [int]$Y = 0,
  [string]$Addr = '0x554480',
  [int]$Length = 64
)

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class BW32 {
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern IntPtr OpenProcess(uint da, bool inherit, int pid);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool ReadProcessMemory(IntPtr h, IntPtr addr, byte[] buf, int size, out IntPtr read);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool CloseHandle(IntPtr h);
}
'@

# --- fixed addresses (docs/battle.md) ---------------------------------------
$FIGURES   = 0x00554480   # g_battleFigures, stride 0x1B0, index 1..80
$FIG_STRIDE = 0x1B0
$UNITS     = 0x00566520   # g_battleUnits, stride 0x34, index 1..80
$UNIT_STRIDE = 0x34
$FIELD     = 0x005440E0   # g_battlefield, 80*80 cells of 8 bytes
$SIZECLASS = 0x00522F6C
$MENPERFIG = 0x004EEA9C
$SIEGEFLAG = 0x0053E8FC
$LOCALPLR  = 0x0057C8CC

$TROOPNAME = @('Peasant', 'Crossbow', 'Mace', 'Sword', 'Pike', 'Archer', 'Knight',
               'Catapult', 'Tower', 'Ram', 'Oil')
$DIRNAME = @('N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW', '-')

function Get-TargetPid {
  if ($ProcId -ne 0) { return $ProcId }
  $p = Get-Process -Name $Name -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($null -eq $p) { throw "no process named '$Name' is running" }
  return $p.Id
}

$script:handle = [IntPtr]::Zero
function Open-Target([int]$targetPid) {
  $script:handle = [BW32]::OpenProcess(0x0010 -bor 0x0400, $false, $targetPid)
  if ($script:handle -eq [IntPtr]::Zero) {
    throw "OpenProcess failed on pid $targetPid (win32 $([Runtime.InteropServices.Marshal]::GetLastWin32Error()))"
  }
}
function Close-Target { if ($script:handle -ne [IntPtr]::Zero) { [BW32]::CloseHandle($script:handle) | Out-Null } }

function Read-Bytes([uint64]$address, [int]$len) {
  $buf = New-Object byte[] $len
  $read = [IntPtr]::Zero
  $ok = [BW32]::ReadProcessMemory($script:handle, [IntPtr][int64]$address, $buf, $len, [ref]$read)
  if (-not $ok) { throw "ReadProcessMemory failed at 0x$($address.ToString('x8'))" }
  return $buf
}
function Read-I32([uint64]$a) { [BitConverter]::ToInt32((Read-Bytes $a 4), 0) }

function U8([byte[]]$b, [int]$o) { return [int]$b[$o] }
function S8([byte[]]$b, [int]$o) { return [int][sbyte]$b[$o] }
function S16([byte[]]$b, [int]$o) { return [BitConverter]::ToInt16($b, $o) }
function S32([byte[]]$b, [int]$o) { return [BitConverter]::ToInt32($b, $o) }

function Show-Globals {
  '  size class    {0}' -f (Read-I32 $SIZECLASS)
  '  men / figure  {0}' -f (Read-I32 $MENPERFIG)
  '  siege flag    {0}' -f (Read-I32 $SIEGEFLAG)
  '  local player  {0}' -f (Read-I32 $LOCALPLR)
}

function Show-Figures {
  'idx own sd unit troop     x   y  st dir  men hits atk def band'
  '--- --- -- ---- -------- --- --- --- --- ---- ---- --- --- ----'
  for ($i = 1; $i -le 80; $i++) {
    $b = Read-Bytes ($FIGURES + $i * $FIG_STRIDE) $FIG_STRIDE
    $owner = U8 $b 0x2C
    if ($owner -eq 0) { continue }
    $t = U8 $b 0x12
    $tn = if ($t -lt 11) { $TROOPNAME[$t] } else { "?$t" }
    $d = U8 $b 0x18
    '{0,3} {1,3} {2,2} {3,4} {4,-8} {5,3} {6,3} {7,3} {8,3} {9,4} {10,4} {11,3} {12,3} {13,4}' -f `
      $i, $owner, (U8 $b 0x17A), (S16 $b 0x178), $tn, (S16 $b 0x20), (S16 $b 0x22), `
      (S8 $b 0x31), $DIRNAME[[Math]::Min($d, 8)], (S16 $b 0x1A0), (S16 $b 0x19A), `
      (S16 $b 0x19C), (U8 $b 0x172), (U8 $b 0x197)
  }
}

function Show-Units {
  'idx own hum sd cat figs first last  x   y  tgtx tgty orders retarg fire'
  '--- --- --- -- --- ---- ----- ---- --- --- ---- ---- ------ ------ ----'
  for ($i = 1; $i -le 80; $i++) {
    $b = Read-Bytes ($UNITS + $i * $UNIT_STRIDE) $UNIT_STRIDE
    if ((U8 $b 0) -eq 0) { continue }
    '{0,3} {1,3} {2,3} {3,2} {4,3} {5,4} {6,5} {7,4} {8,3} {9,3} {10,4} {11,4} {12,6} {13,6} {14,4}' -f `
      $i, (U8 $b 0), (U8 $b 1), (U8 $b 3), (U8 $b 8), (U8 $b 2), (S16 $b 4), (S16 $b 6), `
      (S16 $b 0x1E), (S16 $b 0x20), (S16 $b 0x22), (S16 $b 0x24), (S16 $b 0x1A), `
      (S16 $b 0x14), (U8 $b 0x0F)
  }
}

function Show-Cell([int]$cx, [int]$cy) {
  $b = Read-Bytes ($FIELD + ($cy * 80 + $cx) * 8) 8
  'cell ({0},{1}) at 0x{2:x8}' -f $cx, $cy, ($FIELD + ($cy * 80 + $cx) * 8)
  '  +0 terrain id   {0}' -f (U8 $b 0)
  '  +1 flags        0x{0:x2}   (0x10/0x80 impassable)' -f (U8 $b 1)
  '  +2 flags        0x{0:x2}' -f (U8 $b 2)
  '  +3 graphic      {0}' -f (U8 $b 3)
  '  +4 elevation    {0}' -f (U8 $b 4)
  '  +5 figure       {0}' -f (U8 $b 5)
  '  +6 missile head {0}' -f (U8 $b 6)
  '  +7 surface      {0}' -f (U8 $b 7)
}

$pidToUse = Get-TargetPid
Open-Target $pidToUse
try {
  "Lords2.exe pid $pidToUse"
  switch ($Action) {
    'globals' { Show-Globals }
    'figures' { Show-Figures }
    'units'   { Show-Units }
    'cell'    { Show-Cell $X $Y }
    'raw' {
      $a = if ($Addr -match '^0[xX]') { [Convert]::ToUInt64($Addr.Substring(2), 16) } else { [Convert]::ToUInt64($Addr, 10) }
      $bytes = Read-Bytes $a $Length
      for ($i = 0; $i -lt $bytes.Length; $i += 16) {
        $chunk = $bytes[$i..([Math]::Min($i + 15, $bytes.Length - 1))]
        $hex = ($chunk | ForEach-Object { $_.ToString('x2') }) -join ' '
        $asc = ($chunk | ForEach-Object { if ($_ -ge 32 -and $_ -lt 127) { [char]$_ } else { '.' } }) -join ''
        '{0:x8}  {1,-47}  {2}' -f ($a + $i), $hex, $asc
      }
    }
    default { Show-Globals; ''; Show-Units; ''; Show-Figures }
  }
} finally { Close-Target }
