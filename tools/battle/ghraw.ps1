# Run analyzeHeadless against the lords2 Ghidra project with raw pass-through args.
# Scripts live in ghidra_scripts_battle/ (BDecomp, BRefs, BDump, BCallArg); they all
# take an output file as their first argument, so nothing large lands in the console.
#
#   powershell -File tools/battle/ghraw.ps1 -postScript BDecomp out.c 0048b9c1
#   powershell -File tools/battle/ghraw.ps1 -postScript BRefs r.txt range 566500 566600
#
# Note: analyzeHeadless splits -scriptPath on ';', so only one directory can be given.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot"
$hl = "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat"
$out = Join-Path $PSScriptRoot 'out'
if (-not (Test-Path $out)) { New-Item -ItemType Directory $out | Out-Null }
$a = @("E:\dev\ghidra-projects", "lords2", "-process", "Lords2.exe", "-noanalysis",
       "-scriptPath", "E:\dev\lords2\ghidra_scripts_battle") + $Rest
& $hl $a
