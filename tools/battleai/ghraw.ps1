# Run analyzeHeadless against the lords2 Ghidra project with raw pass-through args.
# Scripts live in ghidra_scripts_battleai/ (BDecomp, BRefs, BDump, BCallArg, BTable);
# they all take an output file as their first argument, so nothing large lands in the
# console. Output goes under tools/battleai/out/, which is gitignored.
#
#   powershell -File tools/battleai/ghraw.ps1 -postScript BDecomp out\h.c 0048a9c7
#   powershell -File tools/battleai/ghraw.ps1 -postScript BTable out\t.txt 4d91b8 5
#
# Note: analyzeHeadless splits -scriptPath on ';', so only one directory can be given.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot"
$hl = "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat"
$out = Join-Path $PSScriptRoot 'out'
if (-not (Test-Path $out)) { New-Item -ItemType Directory $out | Out-Null }
$a = @("E:\dev\ghidra-projects", "lords2", "-process", "Lords2.exe", "-noanalysis",
       "-scriptPath", "E:\dev\lords2\ghidra_scripts_battleai") + $Rest
& $hl $a
