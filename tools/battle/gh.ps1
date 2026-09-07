# Convenience wrapper for one Ghidra headless script. See ghraw.ps1 for several.
#   powershell -File tools/battle/gh.ps1 BDecomp out\seed.c 0048b9c1 0047b8b2
param(
  [Parameter(Mandatory = $true)][string]$Script,
  [Parameter(ValueFromRemainingArguments = $true)][string[]]$ScriptArgs
)
$all = @('-postScript', $Script) + $ScriptArgs
& (Join-Path $PSScriptRoot 'ghraw.ps1') @all
