# Run analyzeHeadless against a Lords2 Ghidra project with raw pass-through args.
# Scripts live in ghidra_scripts_view/ (VBDecomp, VBRefs, VBDump, VBCallArg); each
# takes an output file as its first argument so nothing large lands in the console.
#
#   powershell -File tools/view/ghraw.ps1 -Project lords2 -postScript VBDecomp out.c 0047b8b2
#
# Note: analyzeHeadless splits -scriptPath on ';', so only one directory can be given.
param(
  [string]$Project = "lords2",
  [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest
)
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot"
$hl = "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat"
$out = Join-Path $PSScriptRoot 'out'
if (-not (Test-Path $out)) { New-Item -ItemType Directory $out | Out-Null }
$a = @("E:\dev\ghidra-projects", $Project, "-process", "Lords2.exe", "-noanalysis",
       "-scriptPath", "E:\dev\lords2\ghidra_scripts_view") + $Rest
& $hl $a
