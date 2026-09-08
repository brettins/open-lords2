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

  TWO TIERS, and the second is C16's lesson rather than C14's.

  $CHECKS reads initialised .data: a table has an address, and its bytes are
  the rule. $CODE_CHECKS reads .text, because three of the rules l2-kingdom
  takes from a ruleset are not tables at all - the four AI tax ladders are
  if/else-if chains, and the ale and efficiency bounds are MOV immediates.
  There is no address to point at, which is exactly why they were the last
  constants left hardcoded: there was nothing to transcribe. Reading the
  instruction stream is what makes them checkable, and it needs no more than
  the file does.
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
  # county taken. This is the L2.eng group 85 "From army" term, which
  # kingdom.md sec 12 records as having no writer found.
  #
  # All 102 entries, not the first 32 as before: a ruleset can now replace every
  # row of this table, so every row is worth holding against the binary. The
  # length is fixed by arithmetic and not by this comment - 0x004D8778 + 102 * 4
  # is exactly 0x004D8910, where g_goodSellPrice below begins, which is also why
  # the original's unbounded read lands on a merchant price and gives a free
  # army.
  @{ Name = 'g_armyHappinessCost'; Addr = 0x004D8778; Width = 4; Ref = 'sec 12'
     Expect = @(0,1,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,
                10,11,13,15,17,19,21,23,25,27,29,31,34,37,40,44,48,52,56,60,
                64,68,72,75,78,80,82,84,86,88,90,91,92,93,94,95,96,97,98,99,
                # index 60 is the last rising entry; 61..101 are all 101, so the
                # cost is flat and ruinous past three fifths of a county.
                100,
                101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,
                101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,101,
                101) }

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

  # The "Other counties" happiness term, county +0x16, indexed by tax rate.
  # kingdom.md sec 4.1 read it as 5 - rate and l2-kingdom then inferred
  # min(5 - rate, 0) from the save; both are wrong, and this is the table that
  # says so. Checked at 52 entries rather than 51 so that the trailing zero is
  # pinned too: 0x004D63D8 + 52 * 4 is exactly 0x004D64A8, where
  # g_healthDeltaTable above begins, so the two checks bound each other and the
  # length - which is the second, independent reading of the 0..50 tax ceiling -
  # cannot drift unnoticed.
  @{ Name = 'g_taxHappinessOther'; Addr = 0x004D63D8; Width = 4; Ref = 'sec 4.1'
     Expect = @(0,0,0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,0,0,
                -1,-1,-1,-1, -2,-2,-2,-2, -3,-3,-3,-3,
                -4,-4,-4, -5,-5,-5, -6,-6, -7,-7, -8,-8, -9,
                -10,-11,-12,-13,-14,-15,
                # the 52nd word: a zero no rate can reach, because
                # Tax_IncreaseCounty guards taxRate < 0x32.
                0) }

  # The AI personality records, six ints of each. The base is 0x004D8A58, which
  # is exactly 24 bytes past g_castleFreeArchers, and the stride is 3 x 0x50 =
  # 0xF0. Field +0x00 is the farming style AI_ManageFields dispatches on and
  # +0x04 selects one of the three AI tax ladders; the rest are not identified
  # and are pinned only so a change is noticed.
  #
  # Widths are int32, from the decompiler's own reads: AI_SetTaxRates loads the
  # ladder selector as *(int *)(&g_aiPersonality + (lord * 3 - 3) * 0x50). Note
  # docs/symbols.json names 0x004D8A5C rather than 0x004D8A58, because Ghidra's
  # symbol sits on the selector rather than on the start of the record.
  #
  # All four records now, not lords 1 and 4 alone: crates/l2-kingdom carries the
  # whole table and a ruleset can replace it, so the two middle rows are no
  # longer unchecked. Lords 1..3 all select ladder 2; only lord 4 differs.
  @{ Name = 'g_aiPersonality[lord 1]'; Addr = 0x004D8A58; Width = 4; Ref = 'sec 8.2'
     Expect = @(1, 2, 100, 500, 5, 12) }
  @{ Name = 'g_aiPersonality[lord 2]'; Addr = 0x004D8B48; Width = 4; Ref = 'sec 8.2'
     Expect = @(1, 2, 100, 1000, 10, 10) }
  @{ Name = 'g_aiPersonality[lord 3]'; Addr = 0x004D8C38; Width = 4; Ref = 'sec 8.2'
     Expect = @(0, 2, 200, 1600, 15, 8) }
  # Lord 4, three 0x50-byte rows on, and the only lord using a different ladder.
  @{ Name = 'g_aiPersonality[lord 4]'; Addr = 0x004D8D28; Width = 4; Ref = 'sec 8.2'
     Expect = @(9, 1, 50, 1500, 20, 4) }
  # The new-game starting position, {grain, herd, population, health, health}
  # by starting-wealth setting, stride 0x14. Three rows and a zero fourth.
  #
  # Read because of the herd work in kingdom.md sec 13: row 1 is the row the
  # shipped scenario uses, and three of its five columns turn up in
  # lastturn.sav unchanged - popLast is 417 in all fourteen counties and
  # county +0x254, the herd as Herd_SeasonTick found it, is 95 in all
  # fourteen. The fourth is 65, which crates/l2-scenario carried as
  # STARTING_HEALTH_METER with an [I] saying it was the one number in the
  # reproduction that came from prior art rather than from the binary. It is
  # in the binary, and this is where.
  @{ Name = 'g_startingPosition'; Addr = 0x004DC0D0; Width = 4; Ref = 'sec 13'
     Expect = @(10, 40, 167, 45, 41,
                0, 95, 417, 65, 65,
                500, 330, 1181, 85, 85,
                0, 0, 0, 0, 0) }

  # Where a fifth record would begin, and the evidence that there is not one.
  # kingdom.md sec 2 says the lord byte runs 1..5, but these bytes do not fit
  # the shape: a farm style of 17 where every real record reads 0, 1 or 9, and
  # 5000 in the field the four records read 100, 100, 200 and 50 in. So
  # l2-kingdom stops at four and refuses to answer for lord 5, and this check is
  # what would notice if that reading were ever wrong.
  @{ Name = 'g_aiPersonality[past end]'; Addr = 0x004D8E18; Width = 4; Ref = 'sec 8.2'
     Expect = @(17, 0, 5000, 1, 1, 1) }
)

# ---------------------------------------------------------------------------
# Tier two: rules that are instructions rather than data
# ---------------------------------------------------------------------------
#
# docs/decisions.md C16, applied to rules instead of to Rules_InitConstants'
# globals: when a value is absent from .data, read the code that produces it.
# Three of the rules crates/l2-kingdom now takes from a ruleset have no address
# to read at all - they are immediates in the instruction stream:
#
#   * the four AI tax ladders are four if/else-if chains, which is exactly why
#     kingdom.md sec 8.2 says the ladders exist and does not give them;
#   * the ale step and cap are a MOV ECX / MOV EAX pair;
#   * the efficiency ramp's ceiling is a CMP/MOV pair on a stack slot.
#
# Each check names byte patterns to scan for inside one function and the exact
# ordered list of tagged immediates it must find. An exact list is what makes a
# stray match safe: a false positive fails loudly rather than passing quietly.
$CODE_CHECKS = @(
  # AI_SetTaxRates. The happiness thresholds are CMP EAX, imm8 and the rates are
  # MOV byte ptr [eax + eax*2 + 0x53FA69], imm8 - county +0xB9, the tax rate.
  # The personality comparisons in between use a different encoding, so nothing
  # but the ladders matches.
  @{ Name = 'AI_SetTaxRates ladders'; Addr = 0x0049D638; Len = 1737; Ref = 'sec 8.2'
     Patterns = @(
       @{ Op = @(0x83,0xF8); Imm = 1; Tag = 'below' }
       @{ Op = @(0xC6,0x84,0x40,0x69,0xFA,0x53,0x00); Imm = 1; Tag = 'rate' }
     )
     Expect = @(
       # neutral - unowned counties, eight rungs, the only ladder above 12
       'below 20','rate 0','below 40','rate 1','below 50','rate 2',
       'below 60','rate 3','below 70','rate 4','below 80','rate 6',
       'below 90','rate 8','rate 12',
       # ladder 0 - the greediest
       'below 30','rate 0','below 50','rate 2','below 65','rate 4',
       'below 80','rate 10','rate 15',
       # ladder 1 - the same thresholds, softer rates
       'below 30','rate 0','below 50','rate 1','below 65','rate 3',
       'below 80','rate 7','rate 12',
       # ladder 2 - the gentlest, and the one three of the four lords use
       'below 60','rate 0','below 70','rate 1','below 80','rate 2',
       'below 90','rate 3','below 95','rate 8','rate 10'
     ) }

  # FUN_00428C42, buying ale. MOV EAX, 5 is the "5 - alreadyGiven" clamp; MOV
  # ECX, 10 feeds the IDIV that makes the step, so the step is a tenth of the
  # population and the published "+1 per 20%" is twice too coarse. The rung
  # stores fix the ladder's length at five, which is the same 5 again - one
  # number doing both jobs, which is why l2-kingdom carries one field.
  @{ Name = 'ale ladder'; Addr = 0x00428C42; Len = 365; Ref = 'sec 12'
     Patterns = @(
       @{ Op = @(0xB8); Imm = 4; Tag = 'cap' }
       @{ Op = @(0xB9); Imm = 4; Tag = 'step_divisor' }
       @{ Op = @(0xC7,0x45,0xF8); Imm = 4; Tag = 'rung' }
     )
     Expect = @('cap 5','step_divisor 10',
                'rung 5','rung 4','rung 3','rung 2','rung 1','rung 0','rung 0') }

  # FUN_0044F248, the efficiency ramp. MOV EAX, 0x50 is the flat 80 returned
  # with Advanced Farming off; the CMP/MOV pair on [ebp-0x0C] is the ceiling.
  @{ Name = 'efficiency ramp'; Addr = 0x0044F248; Len = 208; Ref = 'sec 7.4'
     Patterns = @(
       @{ Op = @(0xB8); Imm = 4; Tag = 'flat' }
       @{ Op = @(0x83,0x7D,0xF4); Imm = 1; Tag = 'ceiling_cmp' }
       @{ Op = @(0xC7,0x45,0xF4); Imm = 4; Tag = 'ceiling' }
     )
     Expect = @('flat 80','ceiling_cmp 100','ceiling 100') }

  # FUN_0044D913, the crowding bands of kingdom.md sec 13.1. Two if/else-if
  # chains on the same density in [ebp-8]: the first picks a map graphic
  # 0x13..0x16 into [ebp-4] - so crowding is visible on the field art - and the
  # second writes the level itself to county +0x25C, which is the
  # `MOV dword ptr [eax + 0x53FC0C], imm32` store. The `crowding 40` appears
  # twice because a county with no pasture is pushed to the top band a second
  # time after the chain.
  @{ Name = 'herd crowding bands'; Addr = 0x0044D913; Len = 390; Ref = 'sec 13.1'
     Patterns = @(
       @{ Op = @(0xC7,0x84,0x40,0x0C,0xFC,0x53,0x00); Imm = 4; Tag = 'crowding' }
       @{ Op = @(0x83,0x7D,0xF8); Imm = 1; Tag = 'density_below' }
       @{ Op = @(0xC7,0x45,0xF8); Imm = 4; Tag = 'density' }
       @{ Op = @(0xC7,0x45,0xFC); Imm = 4; Tag = 'graphic' }
     )
     Expect = @(
       'crowding 0',
       'density 0', 'density 1000',
       'graphic 19', 'density_below 10', 'graphic 20', 'density_below 20',
       'graphic 21', 'graphic 22',
       'density_below 10', 'crowding 10',
       'density_below 20', 'crowding 20',
       'density_below 30', 'crowding 30',
       'crowding 40',
       'crowding 40'
     ) }

  # FUN_0044DA99, the rule that a herd has to be tended - kingdom.md sec 13.
  # Six immediate families, in address order:
  #
  #   herd            CMP [ebp+0x0C], imm8  - the herd argument. 0 twice (the
  #                   "any cattle at all" guard and the divide-by-zero guard),
  #                   6 for the no-pasture floor, then 5/10/25 for the small
  #                   herd birth bonus.
  #   staffing_cap    CMP/MOV [ebp-0x14], 200 - twice staffed is the ceiling,
  #                   and the comparison is >= 200 rather than > 199.
  #   full_staffing   CMP [ebp-0x14], 100 - three times: the understaffing
  #                   arm, the small-herd bonus arm, and the birth-rate arm.
  #   crowding        CMP [ebp+0x14], imm8 - 10/20/30 twice over, once for the
  #                   death rate and once for the birth rate.
  #   death_rate      MOV [ebp-0x10], imm32 - 1, 3, 5, 7 per ten thousand.
  #   birth_rate      MOV [ebp-0x0C], imm32 - 1400, 900, 500, 200, and the
  #                   ADD form is the 10000/5000/2000 small-herd bonus.
  #   season          CMP [ebp+0x18], imm8 - 4 kills, 1 calves.
  @{ Name = 'herd births and deaths'; Addr = 0x0044DA99; Len = 692; Ref = 'sec 13'
     Patterns = @(
       @{ Op = @(0x83,0x7D,0x0C); Imm = 1; Tag = 'herd' }
       @{ Op = @(0x81,0x7D,0xEC); Imm = 4; Tag = 'staffing_cap_cmp' }
       @{ Op = @(0xC7,0x45,0xEC); Imm = 4; Tag = 'staffing_cap' }
       @{ Op = @(0x83,0x7D,0xEC); Imm = 1; Tag = 'full_staffing' }
       @{ Op = @(0x83,0x7D,0x14); Imm = 1; Tag = 'crowding' }
       @{ Op = @(0xC7,0x45,0xF0); Imm = 4; Tag = 'death_rate' }
       @{ Op = @(0xC7,0x45,0xF4); Imm = 4; Tag = 'birth_rate' }
       @{ Op = @(0x81,0x45,0xF4); Imm = 4; Tag = 'small_herd_bonus' }
       @{ Op = @(0xB9); Imm = 4; Tag = 'understaffing_divisor' }
       @{ Op = @(0x83,0x7D,0x18); Imm = 1; Tag = 'season' }
     )
     Expect = @(
       'herd 0', 'herd 6', 'herd 0',
       'staffing_cap_cmp 200', 'staffing_cap 200',
       'crowding 10', 'death_rate 1', 'crowding 20', 'death_rate 3',
       'crowding 30', 'death_rate 5', 'death_rate 7',
       'full_staffing 100', 'understaffing_divisor 3',
       'crowding 10', 'birth_rate 1400', 'crowding 20', 'birth_rate 900',
       'crowding 30', 'birth_rate 500', 'birth_rate 200',
       'full_staffing 100',
       'full_staffing 100',
       'herd 5', 'small_herd_bonus 10000',
       'herd 10', 'small_herd_bonus 5000',
       'herd 25', 'small_herd_bonus 2000',
       'season 4',
       'season 1'
     ) }
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

# Walk a function's bytes once, in address order, emitting "<tag> <value>" for
# every immediate whose opcode prefix matches one of $patterns.
#
# Deliberately a scan and not a disassembler: it cannot tell an instruction
# boundary from a byte inside an operand, so a stray match is possible - and
# harmless, because the caller compares the whole ordered list. A false positive
# fails the check rather than passing it quietly, which is the failure mode to
# have.
function Read-Immediates($bytes, $patterns) {
  $out = @()
  for ($i = 0; $i -lt $bytes.Length; $i++) {
    foreach ($p in $patterns) {
      $n = $p.Op.Count
      if ($i + $n + $p.Imm -gt $bytes.Length) { continue }
      $hit = $true
      for ($j = 0; $j -lt $n; $j++) {
        if ($bytes[$i + $j] -ne $p.Op[$j]) { $hit = $false; break }
      }
      if (-not $hit) { continue }
      $value = if ($p.Imm -eq 1) { [int][sbyte]$bytes[$i + $n] }
               else { [BitConverter]::ToInt32($bytes, $i + $n) }
      $out += "$($p.Tag) $value"
      $i += $n + $p.Imm - 1
      break
    }
  }
  return ,$out
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

  foreach ($c in $CODE_CHECKS) {
    $bytes = if ($Source -eq 'Live') { Read-Live $targetPid ([uint32]$c.Addr) $c.Len }
             else { Read-Bytes $image ([uint32]$c.Addr) $c.Len }
    $got = Read-Immediates $bytes $c.Patterns
    $same = -not (Compare-Object $got $c.Expect -SyncWindow 0)
    if ($same) {
      $pass++
      Write-Host ("  PASS  {0,-24} {1}  {2}" -f $c.Name, $c.Ref, ($got -join ', ')) -ForegroundColor Green
    } else {
      $fail++
      Write-Host ("  FAIL  {0,-24} {1}" -f $c.Name, $c.Ref) -ForegroundColor Red
      Write-Host ("        l2-kingdom says: " + ($c.Expect -join ', ')) -ForegroundColor DarkGray
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
