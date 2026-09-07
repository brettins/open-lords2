# Run analyzeHeadless against the lords2 Ghidra project with raw pass-through args.
# Scripts live in tools/kingdom/ghidra (KDecomp, KRefs, KDump, KCallArg); they all take
# an output file as their first argument, so nothing large lands in the console.
# Output files are written into tools/kingdom/out/, which is gitignored.
#
#   powershell -File tools/kingdom/ghraw.ps1 -postScript KDecomp turn.c 0049a010
#   powershell -File tools/kingdom/ghraw.ps1 -postScript KRefs   r.txt range 56d500 56d600
#
# Note: analyzeHeadless splits -scriptPath on ';', so only one directory can be given.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot"
$hl = "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat"
$out = Join-Path $PSScriptRoot 'out'
if (-not (Test-Path $out)) { New-Item -ItemType Directory $out | Out-Null }
$a = @("E:\dev\ghidra-projects", "lords2", "-process", "Lords2.exe", "-noanalysis",
       "-scriptPath", (Join-Path $PSScriptRoot 'ghidra')) + $Rest
Push-Location $out
try { & $hl $a } finally { Pop-Location }
