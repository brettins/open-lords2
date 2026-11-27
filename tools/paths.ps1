# Dot-source: . "$PSScriptRoot\..\paths.ps1"   (paths.local.example)
$script:L2Root = Split-Path -Parent $PSScriptRoot

function Get-L2Path([string]$Key, [switch]$Optional) {
  $env = [Environment]::GetEnvironmentVariable($Key)
  if ($env) { return $env.TrimEnd('\', '/') }
  $file = Join-Path $script:L2Root 'paths.local'
  if (Test-Path $file) {
    foreach ($line in Get-Content $file) {
      if ($line -match '^\s*([A-Z0-9_]+)\s*=\s*(.+?)\s*$' -and $Matches[1] -eq $Key -and (Test-Path $Matches[2])) {
        return $Matches[2].TrimEnd('\', '/')
      }
    }
  }
  if ($Optional) { return $null }
  throw "$Key is not set: set it in the environment, or add it to paths.local (see paths.local.example)."
}
