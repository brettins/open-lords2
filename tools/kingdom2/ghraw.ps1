# Run analyzeHeadless against a private Ghidra project for the kingdom2 agent.
# Separate project directory so this cannot collide with a parallel agent holding
# the shared `lords2` project (docs/agents.md: one process per Ghidra project).
#
#   powershell -File tools/kingdom2/ghraw.ps1 -postScript KDecomp turn.c 0049a010
#
# Output files land in tools/kingdom2/out/, which is gitignored - decompiler
# output is derived from the copyrighted binary and must never be committed.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot"
$hl = "E:\dev\tools\ghidra_12.1.3_PUBLIC\support\analyzeHeadless.bat"
$out = Join-Path $PSScriptRoot 'out'
if (-not (Test-Path $out)) { New-Item -ItemType Directory $out | Out-Null }
$a = @("E:\dev\ghidra-projects-kingdom2", "l2k2", "-process", "Lords2.exe", "-noanalysis",
       "-scriptPath", (Join-Path $PSScriptRoot 'ghidra')) + $Rest
Push-Location $out
try { & $hl $a } finally { Pop-Location }
