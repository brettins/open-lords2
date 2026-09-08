<#
  initconsts.ps1 - recover Rules_InitConstants' values without running the game.

  This replaces the live read in runtime.ps1, and the story of why is worth
  keeping.

  The constants Rules_InitConstants writes live in uninitialised .data, so
  reading the executable *at those addresses* returns nothing - the file has no
  bytes there. From that I concluded the values could only be had from a running
  process, built a launcher that hands the game the foreground, and read them
  out of live memory.

  That was true and beside the point. The values are not in .data, but they are
  in .text: the function writes them with `MOV dword ptr [addr], imm32`, and the
  immediate is right there in the instruction stream. Disassembling the writes
  gives the same numbers with no process, no window, no focus, and no way for a
  screen lock to spoil the run.

  Verified equal: all ten constants recovered here match what the live read
  returned, value for value. And the scan finds five more that nobody had
  named - a live read only tells you about addresses you already knew to look
  at, while the function tells you everything it writes.

  The encoding, for anyone extending this:

      C7 05 <addr:u32> <imm:u32>     MOV dword ptr [addr], imm32

  C7 /0 is the "move immediate to r/m32" form; modrm 05 means the operand is a
  bare 32-bit absolute address, which is what a global looks like in a non-PIC
  x86 binary. Scanning stops at the first RET.
#>
[CmdletBinding()]
param(
  [string]$Exe = 'F:\games\Lords of the Realm II\Lords2.exe',
  [uint32]$Function = 0x004983B7,
  [int]$MaxBytes = 600
)

$ErrorActionPreference = 'Stop'
$IMAGE_BASE = 0x400000

# addr -> expected value and name, from docs/symbols.json and docs/battle-ai.md.
$KNOWN = @{
  0x0057C8B4 = @{ Name = 'g_aiAggressionThreshold'; Expect = 5 }
  0x00552FF0 = @{ Name = 'g_aiSortieThreshold';     Expect = 260 }
  0x00554090 = @{ Name = 'g_moatFillSteps';         Expect = 15 }
  0x0057C8E0 = @{ Name = 'g_grainYieldPerSack';     Expect = 12 }
  0x00552FFC = @{ Name = 'g_grainMaxSacksPerField'; Expect = 10 }
  0x00567594 = @{ Name = 'g_foodPerHead';           Expect = 10 }
  0x0057CB30 = @{ Name = 'g_foodPerSack';           Expect = 6 }
  0x00553F60 = @{ Name = 'g_dairyPerHead';          Expect = 5 }
  0x0057D34C = @{ Name = 'g_grainLabourDivisor';    Expect = 2 }
  0x005533BC = @{ Name = 'g_grainLabourDivisorAdv'; Expect = 5 }
}

$bytes = [System.IO.File]::ReadAllBytes($Exe)
$peOff = [BitConverter]::ToInt32($bytes, 0x3C)
$nSec = [BitConverter]::ToUInt16($bytes, $peOff + 6)
$optSize = [BitConverter]::ToUInt16($bytes, $peOff + 20)
$secOff = $peOff + 24 + $optSize
$sections = @()
for ($i = 0; $i -lt $nSec; $i++) {
  $o = $secOff + $i * 40
  $sections += [pscustomobject]@{
    Name  = ([System.Text.Encoding]::ASCII.GetString($bytes, $o, 8)).TrimEnd([char]0)
    VSize = [BitConverter]::ToUInt32($bytes, $o + 8);  VAddr = [BitConverter]::ToUInt32($bytes, $o + 12)
    RawSz = [BitConverter]::ToUInt32($bytes, $o + 16); RawPtr = [BitConverter]::ToUInt32($bytes, $o + 20)
  }
}
function Convert-VaToOffset([uint32]$va) {
  $rva = $va - $IMAGE_BASE
  foreach ($s in $sections) {
    if ($rva -ge $s.VAddr -and $rva -lt ($s.VAddr + [Math]::Max($s.VSize, $s.RawSz))) {
      return $s.RawPtr + ($rva - $s.VAddr)
    }
  }
  throw "0x$($va.ToString('x8')) is in no section"
}

Write-Host "oracle: $Exe" -ForegroundColor Green
Write-Host ("scanning 0x{0:x8} for MOV dword ptr [abs], imm32" -f $Function)
Write-Host ""

$start = Convert-VaToOffset $Function
$writes = @()
for ($i = 0; $i -lt $MaxBytes; $i++) {
  $p = $start + $i
  if ($bytes[$p] -eq 0xC7 -and $bytes[$p + 1] -eq 0x05) {
    $writes += [pscustomobject]@{
      Addr = [BitConverter]::ToUInt32($bytes, $p + 2)
      Value = [BitConverter]::ToInt32($bytes, $p + 6)
    }
    $i += 9
    continue
  }
  if ($bytes[$p] -eq 0xC3) { break }   # RET
}

$pass = 0; $fail = 0; $unknown = 0
foreach ($w in $writes) {
  $k = $KNOWN[[int]$w.Addr]
  if ($null -eq $k) {
    $unknown++
    Write-Host ("  ----  0x{0:x8} = {1,-6} (not in symbols.json)" -f $w.Addr, $w.Value) -ForegroundColor DarkGray
  } elseif ($k.Expect -eq $w.Value) {
    $pass++
    Write-Host ("  PASS  {0,-26} = {1,-6} @ 0x{2:x8}" -f $k.Name, $w.Value, $w.Addr) -ForegroundColor Green
  } else {
    $fail++
    Write-Host ("  FAIL  {0,-26} expected {1}, binary writes {2}" -f $k.Name, $k.Expect, $w.Value) -ForegroundColor Red
  }
}

$missing = $KNOWN.Keys | Where-Object { $a = $_; -not ($writes | Where-Object { $_.Addr -eq $a }) }
foreach ($m in $missing) {
  $fail++
  Write-Host ("  MISS  {0,-26} no write found at 0x{1:x8}" -f $KNOWN[$m].Name, $m) -ForegroundColor Red
}

Write-Host ""
Write-Host "$pass passed, $fail failed, $unknown unnamed constants found" -ForegroundColor $(if ($fail) { 'Red' } else { 'Green' })
Write-Host "no process was launched" -ForegroundColor Cyan
if ($fail) { exit 1 }
