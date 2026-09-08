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

  Re-run it after docs/symbols.json changes so names propagate. Naming one
  global makes dozens of unrelated functions legible, and this is how that
  reaches code nobody has opened yet.

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
if (-not $SkipSymbols) {
  Write-Host "applying docs/symbols.json to the Ghidra database..." -ForegroundColor Cyan
  & $Headless $GhidraProject $ProjectName -process 'Lords2.exe' -noanalysis `
      -scriptPath $scripts -postScript ApplySymbols 2>&1 |
    Select-String -Pattern 'ApplySymbols|ERROR|failed' | Select-Object -First 20
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
