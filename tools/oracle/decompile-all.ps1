<#
  decompile-all.ps1 - decompile the whole binary once, into something greppable.

  The project's habit until now was to start Ghidra per question. Six per-task
  out/ directories accumulated that way, about 6.4 MB of overlapping fragments,
  and every new question paid the analysis cost again.

  Producing the C is the cheap part. Understanding it is the expensive part, and
  re-deriving it does not help. So this runs once and everything downstream
  greps the result:

      rg "g_pathCost" tools/oracle/decomp/          # who touches this global
      rg "0x004d96d0" tools/oracle/decomp/          # who reads this table
      rg -l "Rules_InitConstants" tools/oracle/decomp/

  Re-run it after docs/symbols.json or docs/records.json changes so names
  propagate. Naming one global makes dozens of unrelated functions legible, and
  this is how that reaches code nobody has opened yet.

  Output goes to tools/oracle/decomp/, which is gitignored by the **/out/ and
  explicit rules - it is derived from the shipped binary and must never be
  committed. Applying symbols first is the default; -SkipSymbols skips it.
#>
[CmdletBinding()]
param(
  [string]$OutDir = "$PSScriptRoot\decomp",
  [switch]$SkipSymbols,
  [string]$GhidraProject = 'E:\dev\ghidra-projects',
  [string]$ProjectName = 'lords2',
  [string]$Headless = 'E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat',
  [string]$Jdk = 'C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot'
)

$ErrorActionPreference = 'Stop'
$env:JAVA_HOME = $Jdk
$scripts = Join-Path (Split-Path (Split-Path $PSScriptRoot)) 'ghidra_scripts'

if (-not (Test-Path $Headless)) { throw "analyzeHeadless not found at $Headless" }
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Force $OutDir | Out-Null }

# Names first, so the decompilation reads in terms of g_pathCost rather than
# DAT_00504030. This is most of what makes the output worth having.
# **A dropped symbol must stop this script.** ApplySymbols prints one line per
# entry it could not apply and then carries on, so the corpus comes out looking
# complete while a name is missing from it. That happened twice in two days - a
# calling convention in a signature, then a data table filed under "functions" -
# and both times the pipeline reported success and an integrator happened to read
# the output. `tools/symbols/symbols_md.js` now catches that particular species
# before Ghidra sees it; this catches every other one, which is why both exist.
function Invoke-Apply {
  param([string]$Script, [string]$What)
  Write-Host "applying $What to the Ghidra database..." -ForegroundColor Cyan
  $out = & $Headless $GhidraProject $ProjectName -process 'Lords2.exe' -noanalysis `
      -scriptPath $scripts -postScript $Script 2>&1
  $out | Select-String -Pattern "$Script|ERROR|failed" | Select-Object -First 20

  # ApplySymbols reports the same failure on TWO lines - its summary
  # ("... failed 1") and its error line ("### 1 entry failed") - so take the
  # largest count seen rather than the sum, which counted every failure twice.
  $failed = 0
  foreach ($line in $out) {
    $n = 0
    if     ("$line" -match 'failed\s+(\d+)')             { $n = [int]$Matches[1] }
    elseif ("$line" -match '(\d+)\s+entr(y|ies) failed') { $n = [int]$Matches[1] }
    if ($n -gt $failed) { $failed = $n }
  }
  if ($failed -gt 0) {
    Write-Host ""
    Write-Host "$Script could not apply $failed entr$(if ($failed -eq 1) {'y'} else {'ies'})." -ForegroundColor Red
    Write-Host "The lines above name each one. A dropped entry means the corpus is missing" -ForegroundColor Red
    Write-Host "that name on EVERY rebuild, silently - which is why this is fatal rather" -ForegroundColor Red
    Write-Host "than a warning." -ForegroundColor Red
    Write-Host ""
    Write-Host "  'Can't resolve return type'   the signature carries a calling convention;" -ForegroundColor Yellow
    Write-Host "                                delete it, Ghidra infers it." -ForegroundColor Yellow
    Write-Host "  'no function at <addr>'       the entry is data in the functions array;" -ForegroundColor Yellow
    Write-Host "                                move it to globals and drop its signature." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  node tools/symbols/symbols_md.js --check   catches both of those, and runs" -ForegroundColor Yellow
    Write-Host "                                             without Ghidra." -ForegroundColor Yellow
    exit 1
  }
}

if (-not $SkipSymbols) {
  Invoke-Apply -Script 'ApplySymbols' -What 'docs/symbols.json'

  # Then the record layouts. Without them the decompiler invents one global per
  # field of every record array - 334 of them - and a third of the binary reads
  # as (&DAT_0053f9bc)[i * 0x300] instead of g_counties[i].happiness.
  Invoke-Apply -Script 'ApplyRecords' -What 'docs/records.json struct layouts'
}

Write-Host "decompiling every function - this takes a while, once..." -ForegroundColor Cyan
$sw = [Diagnostics.Stopwatch]::StartNew()
& $Headless $GhidraProject $ProjectName -process 'Lords2.exe' -noanalysis `
    -scriptPath $scripts -postScript DecompileAll $OutDir 2>&1 |
  Select-String -Pattern 'DecompileAll|decompiled \d+ functions|ERROR' |
  Select-Object -Last 20
$sw.Stop()

Write-Host ""
$files = Get-ChildItem $OutDir -Filter *.c -ErrorAction SilentlyContinue
$bytes = ($files | Measure-Object -Property Length -Sum).Sum
$index = Join-Path $OutDir 'index.txt'
$funcs = if (Test-Path $index) { (Get-Content $index | Where-Object { $_ -notmatch '^#' }).Count } else { 0 }
Write-Host ("{0} files, {1:N0} functions, {2:N1} MB, in {3:N0}s" -f `
  $files.Count, $funcs, ($bytes / 1MB), $sw.Elapsed.TotalSeconds) -ForegroundColor Green
Write-Host "grep it: rg <pattern> $OutDir" -ForegroundColor Cyan
